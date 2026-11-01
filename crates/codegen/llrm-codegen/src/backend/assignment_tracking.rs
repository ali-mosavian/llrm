//! `AssignmentTrackingAnalysis`: where each variable of a function lives, from
//! its `#dbg_assign` records and the stores they name, as a table of locations
//! the code generator emits.
//!
//! # How it works
//! [`FunctionVarLocs`] is the result, built in [`FunctionVarLocsBuilder`]:
//! the variables (a variable and the bit range of it a location covers), the
//! variables that have one location for their whole scope, and the locations
//! that start just before an instruction. [`build_overlap_map`] finds which
//! fragments contain which and which stores write them; [`Lowering`] is the
//! dataflow: a state per block ([`BlockInfo`]), transfer functions for stores,
//! escaping calls and records, and `run`, the fixpoint over the blocks.
//!
//! # Example
//! A variable `x` kept in `%a` from its first use, and `y` in its stack slot
//! for the whole function:
//!
//! ```text
//! === Variables ===
//! [1] x
//! [2] y
//! === Single location vars ===
//! DEF Var=[2] Expr=!DIExpression(DW_OP_deref) Values=(%slot )
//! === In-line variable defs ===
//! DEF Var=[1] Expr=!DIExpression() Values=(%a )
//! ```
//!
//! # Complexity
//! Linear in the locations: a variable is found by a hash of its key, a wedge
//! by its instruction. The analysis that builds it keeps the pin's bound (a
//! fixpoint over blocks and variables, cut off at `MAX_BLOCKS`).
//!
//! # See also
//! `llvm/lib/CodeGen/AssignmentTrackingAnalysis.cpp`, pin d1106deb71cc.

mod fill;
mod redundant;
mod rewrite;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write;

use llrm_mir::types::Type;
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::nonzero::{get_pointer_offset_from, strip_and_accumulate_constant_offsets};
use llrm_mir::{
    ConstantKind, DebugRecord, DebugWhat, DwOp, Function, InstId, MetadataId, Module, Opcode, Operand, ValueDef,
    ValueId,
};
pub use rewrite::rewrite;

/// Where a location starts: just before record `slot` of `inst`, or before
/// `inst` itself when `slot` is the number of records standing before it
/// (`VarLocInsertPt`, a `DbgRecord` or an `Instruction`).
pub type InsertPt = (InstId, u32);

/// A number of a variable in [`FunctionVarLocs::variables`]; 0 is reserved
/// (`VariableID::Reserved`), as the pin's table is one-based.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VariableId(pub u32);

/// The bits of a variable a location covers (`DIExpression::FragmentInfo`).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Fragment {
    pub offset: u64,
    pub size: u64,
}

/// A variable, or a fragment of it, as one inlined instance of it
/// (`DebugVariable`).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DebugVariable {
    /// The variable's node (`DILocalVariable`).
    pub variable: MetadataId,
    pub fragment: Option<Fragment>,
    /// The `DILocation` of the call this instance was inlined at.
    pub inlined_at: Option<MetadataId>,
}

impl DebugVariable {
    /// `getFragmentOrDefault`: a variable with no fragment is all bits, up
    /// to `u32::MAX`.
    pub fn fragment_or_default(&self) -> Fragment {
        self.fragment.unwrap_or(Fragment { offset: 0, size: u64::from(u32::MAX) })
    }
}

/// One definition: the variable is what the expression makes of the values,
/// from this point (`VarLocInfo`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VarLocInfo {
    pub variable: VariableId,
    pub expression: Vec<DwOp>,
    pub location: Option<MetadataId>,
    /// The values the expression reads; none for a location nothing says.
    pub values: Vec<Operand>,
}

/// The kind of location in use for a variable: `Mem` is the stack home, `Val`
/// an SSA value or constant, and `None` that there is not one single kind
/// (several, or none). A join-semilattice, `None > Mem, Val`: the join of two
/// different kinds is `None` (`LocKind`, `joinKind`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocKind {
    Mem,
    Val,
    None,
}

impl LocKind {
    pub fn join(
        self,
        other: LocKind,
    ) -> LocKind {
        if self == other { self } else { LocKind::None }
    }
}

/// The assignment of a value to a variable or to memory, named by its
/// `!DIAssignID` (`Assignment`). `NoneOrPhi` is the join of two different
/// ones, or none known; `Known` has an id. A join-semilattice,
/// `NoneOrPhi > Known(id)`.
#[derive(Clone, Copy, Debug)]
pub enum Assignment {
    Known {
        id: MetadataId,
        /// The `#dbg_assign` that marks this definition; none for a memory
        /// definition, which has no use for it.
        source: Option<DebugRecord>,
    },
    NoneOrPhi,
}

impl Assignment {
    /// Assignments are defined by their id, not by the records that name it
    /// (`isSameSourceAssignment`).
    pub fn is_same_source_assignment(
        &self,
        other: &Assignment,
    ) -> bool {
        match (self, other) {
            (Assignment::Known { id: a, .. }, Assignment::Known { id: b, .. }) => a == b,
            (Assignment::NoneOrPhi, Assignment::NoneOrPhi) => true,
            _ => false,
        }
    }

    /// `joinAssignment`: two different assignments, or an unknown one, join
    /// to `NoneOrPhi`. Where the same one comes from two records, the record
    /// is kept only if they are equivalent: there is no telling which of two
    /// constants to take.
    pub fn join(
        &self,
        other: &Assignment,
    ) -> Assignment {
        match (self, other) {
            (Assignment::Known { id, source: a }, Assignment::Known { id: other_id, source: b }) if id == other_id => {
                let source = match (a, b) {
                    (Some(a), Some(b)) if records_equivalent(a, b) => Some(*a),
                    _ => None,
                };
                Assignment::Known { id: *id, source }
            }
            _ => Assignment::NoneOrPhi,
        }
    }
}

/// `DbgVariableRecord::isEquivalentTo`: the same statement, wherever it stands.
fn records_equivalent(
    a: &DebugRecord,
    b: &DebugRecord,
) -> bool {
    (a.variable, a.what, a.expression, a.location) == (b.variable, b.what, b.expression, b.location)
}

/// What is known at a point of the function about the variables it has
/// seen (`BlockInfo`): per variable the assignment last made to its stack
/// home, the one last made to its debug value, and which of the two is live.
#[derive(Clone, Debug, Default)]
pub struct BlockInfo {
    tracked: Vec<bool>,
    stack_home: Vec<Assignment>,
    debug_value: Vec<Assignment>,
    live: Vec<LocKind>,
}

/// Which map of a [`BlockInfo`] (`AssignmentKind`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssignmentKind {
    Stack,
    Debug,
}

impl BlockInfo {
    /// A block that tracks nothing, with room for `variables` variables
    /// (`init`); index 0 is the reserved variable.
    pub fn new(variables: usize) -> BlockInfo {
        BlockInfo {
            tracked: vec![false; variables],
            stack_home: vec![Assignment::NoneOrPhi; variables],
            debug_value: vec![Assignment::NoneOrPhi; variables],
            live: vec![LocKind::None; variables],
        }
    }

    fn map(
        &self,
        kind: AssignmentKind,
    ) -> &[Assignment] {
        match kind {
            AssignmentKind::Stack => &self.stack_home,
            AssignmentKind::Debug => &self.debug_value,
        }
    }

    pub fn is_variable_tracked(
        &self,
        variable: VariableId,
    ) -> bool {
        self.tracked[variable.0 as usize]
    }

    pub fn assignment(
        &self,
        kind: AssignmentKind,
        variable: VariableId,
    ) -> &Assignment {
        assert!(self.is_variable_tracked(variable), "the variable is not tracked in this block");
        &self.map(kind)[variable.0 as usize]
    }

    pub fn loc_kind(
        &self,
        variable: VariableId,
    ) -> LocKind {
        assert!(self.is_variable_tracked(variable), "the variable is not tracked in this block");
        self.live[variable.0 as usize]
    }

    pub fn set_loc_kind(
        &mut self,
        variable: VariableId,
        kind: LocKind,
    ) {
        self.tracked[variable.0 as usize] = true;
        self.live[variable.0 as usize] = kind;
    }

    pub fn set_assignment(
        &mut self,
        kind: AssignmentKind,
        variable: VariableId,
        assignment: Assignment,
    ) {
        self.tracked[variable.0 as usize] = true;
        match kind {
            AssignmentKind::Stack => self.stack_home[variable.0 as usize] = assignment,
            AssignmentKind::Debug => self.debug_value[variable.0 as usize] = assignment,
        }
    }

    pub fn has_assignment(
        &self,
        kind: AssignmentKind,
        variable: VariableId,
        assignment: &Assignment,
    ) -> bool {
        self.is_variable_tracked(variable) && assignment.is_same_source_assignment(self.assignment(kind, variable))
    }

    /// `BlockInfo::join`: for a variable both track, the join of each of the
    /// three; the tracked variables are those either tracks.
    pub fn join(
        a: &BlockInfo,
        b: &BlockInfo,
        variables: usize,
    ) -> BlockInfo {
        let mut join = BlockInfo::new(variables);
        for variable in 0..variables {
            if a.tracked[variable] && b.tracked[variable] {
                join.live[variable] = a.live[variable].join(b.live[variable]);
                join.debug_value[variable] = a.debug_value[variable].join(&b.debug_value[variable]);
                join.stack_home[variable] = a.stack_home[variable].join(&b.stack_home[variable]);
            }
            join.tracked[variable] = a.tracked[variable] || b.tracked[variable];
        }
        join
    }
}

/// Equal where the tracked variables, their live kinds and the assignments'
/// ids are (`BlockInfo::operator==`): the records an assignment came from do
/// not count.
impl PartialEq for BlockInfo {
    fn eq(
        &self,
        other: &BlockInfo,
    ) -> bool {
        self.tracked == other.tracked
            && self.live == other.live
            && (0..self.tracked.len()).filter(|&at| self.tracked[at]).all(|at| {
                self.stack_home[at].is_same_source_assignment(&other.stack_home[at])
                    && self.debug_value[at].is_same_source_assignment(&other.debug_value[at])
            })
    }
}

/// The result: the locations of one function (`FunctionVarLocs`).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FunctionVarLocs {
    variables: Vec<DebugVariable>,
    single: Vec<VarLocInfo>,
    before: BTreeMap<InstId, Vec<VarLocInfo>>,
}

/// What [`FunctionVarLocs`] is built in (`FunctionVarLocsBuilder`).
#[derive(Debug, Default)]
pub struct FunctionVarLocsBuilder {
    variables: Vec<DebugVariable>,
    ids: HashMap<DebugVariable, VariableId>,
    single: Vec<VarLocInfo>,
    before: BTreeMap<InsertPt, Vec<VarLocInfo>>,
}

impl FunctionVarLocsBuilder {
    pub fn number_of_variables(&self) -> usize {
        self.variables.len()
    }

    /// The id of `variable`, new if it has none (`insertVariable`).
    pub fn insert_variable(
        &mut self,
        variable: DebugVariable,
    ) -> VariableId {
        if let Some(&id) = self.ids.get(&variable) {
            return id;
        }
        self.variables.push(variable);
        let id = VariableId(self.variables.len() as u32);
        self.ids.insert(variable, id);
        id
    }

    pub fn variable(
        &self,
        id: VariableId,
    ) -> &DebugVariable {
        &self.variables[id.0 as usize - 1]
    }

    /// The locations that start just before `inst`, none if there are none.
    pub fn wedge(
        &self,
        inst: InsertPt,
    ) -> Option<&[VarLocInfo]> {
        self.before.get(&inst).map(Vec::as_slice)
    }

    /// Replaces the locations that start just before `inst` (`setWedge`).
    pub fn set_wedge(
        &mut self,
        inst: InsertPt,
        wedge: Vec<VarLocInfo>,
    ) {
        self.before.insert(inst, wedge);
    }

    /// A location valid for the variable's whole scope (`addSingleLocVar`).
    pub fn add_single_loc_var(
        &mut self,
        variable: DebugVariable,
        expression: Vec<DwOp>,
        location: Option<MetadataId>,
        values: Vec<Operand>,
    ) {
        let variable = self.insert_variable(variable);
        self.single.push(VarLocInfo { variable, expression, location, values });
    }

    /// A location that starts just before `before` (`addVarLoc`).
    pub fn add_var_loc(
        &mut self,
        before: InsertPt,
        variable: DebugVariable,
        expression: Vec<DwOp>,
        location: Option<MetadataId>,
        values: Vec<Operand>,
    ) {
        let variable = self.insert_variable(variable);
        self.before.entry(before).or_default().push(VarLocInfo { variable, expression, location, values });
    }

    /// The result (`FunctionVarLocs::init`).
    pub fn finish(self) -> FunctionVarLocs {
        // The locations of the records standing before an instruction come
        // first, in record order, then its own (`FunctionVarLocs::init`).
        let mut before: BTreeMap<InstId, Vec<VarLocInfo>> = BTreeMap::new();
        for ((inst, _), locs) in self.before {
            if !locs.is_empty() {
                before.entry(inst).or_default().extend(locs);
            }
        }
        FunctionVarLocs { variables: self.variables, single: self.single, before }
    }
}

impl FunctionVarLocs {
    pub fn variable(
        &self,
        id: VariableId,
    ) -> &DebugVariable {
        &self.variables[id.0 as usize - 1]
    }

    /// The locations valid for a variable's whole scope.
    pub fn single_locs(&self) -> &[VarLocInfo] {
        &self.single
    }

    /// The locations that start just before `inst`.
    pub fn locs_before(
        &self,
        inst: InstId,
    ) -> &[VarLocInfo] {
        self.before.get(&inst).map_or(&[], Vec::as_slice)
    }

    /// The instructions with locations before them, in order.
    pub fn instructions(&self) -> impl Iterator<Item = InstId> + '_ {
        self.before.keys().copied()
    }

    /// `FunctionVarLocs::print` for the parts that do not need the function:
    /// the variable table, the single locations and the locations by
    /// instruction. `name` says a value and `variable` a variable's name.
    pub fn print(
        &self,
        module: &Module,
        variable_name: &dyn Fn(MetadataId) -> String,
        value_name: &dyn Fn(Operand) -> String,
    ) -> String {
        let mut out = String::from("=== Variables ===\n");
        for (at, one) in self.variables.iter().enumerate() {
            let _ = write!(out, "[{}] {}", at + 1, variable_name(one.variable));
            if let Some(fragment) = one.fragment {
                let _ = write!(out, " bits [{}, {})", fragment.offset, fragment.offset + fragment.size);
            }
            if let Some(inlined) = one.inlined_at {
                let _ = write!(out, " inlined-at !{}", inlined.0);
            }
            out.push('\n');
        }
        let line = |out: &mut String, one: &VarLocInfo| {
            let _ = write!(
                out,
                "DEF Var=[{}] Expr=!DIExpression({}) Values=(",
                one.variable.0,
                expression_text(&one.expression)
            );
            for value in &one.values {
                let _ = write!(out, "{} ", value_name(*value));
            }
            out.push_str(")\n");
        };
        let _ = module;
        out.push_str("=== Single location vars ===\n");
        for one in &self.single {
            line(&mut out, one);
        }
        out.push_str("=== In-line variable defs ===\n");
        for (inst, wedge) in &self.before {
            let _ = writeln!(out, "before %{}:", inst.0);
            for one in wedge {
                line(&mut out, one);
            }
        }
        out
    }
}

/// The bits of the variable a trailing `DW_OP_LLVM_fragment` names
/// (`DIExpression::getFragmentInfo`).
pub fn fragment_of(ops: &[DwOp]) -> Option<Fragment> {
    ops.iter()
        .rev()
        .find_map(
            |op| match *op {
                DwOp::Fragment { offset, size } => Some(Fragment { offset, size }),
                _ => None,
            },
        )
}

/// `DIExpression::extractLeadingOffset`: the bytes the leading
/// `plus_uconst`/`constu`+`plus|minus` operations add, and the operations
/// after them. None when an operation is none of those or a `deref` or fragment
/// (which end the offset), or the expression reads more than one location.
pub fn extract_leading_offset(ops: &[DwOp]) -> Option<(i64, &[DwOp])> {
    let ops = match ops {
        [DwOp::Arg(0), rest @ ..] => rest,
        _ => ops,
    };
    if ops.iter().any(|op| matches!(op, DwOp::Arg(_))) {
        return None;
    }
    let mut offset = 0_i64;
    let mut at = 0;
    while let Some(op) = ops.get(at) {
        match *op {
            DwOp::Deref | DwOp::Fragment { .. } => break,
            DwOp::PlusUconst(n) => offset = offset.wrapping_add(n as i64),
            DwOp::Constu(n) => {
                at += 1;
                match ops.get(at) {
                    Some(DwOp::Binary(llrm_mir::module::DwBinary::Plus)) => offset = offset.wrapping_add(n as i64),
                    Some(DwOp::Binary(llrm_mir::module::DwBinary::Minus)) => offset = offset.wrapping_sub(n as i64),
                    _ => return None,
                }
            }
            _ => return None,
        }
        at += 1;
    }
    Some((offset, &ops[at..]))
}

/// What a store writes: bytes `[offset, offset + size)` of an alloca
/// (`at::AssignmentInfo`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssignmentInfo {
    pub base: ValueId,
    /// The alloca's instruction.
    pub alloca: InstId,
    pub offset_in_bits: u64,
    pub size_in_bits: u64,
}

/// `at::getAssignmentInfo` for a store or a `memcpy`/`memmove`/`memset` of
/// constant length: None when its destination is no constant offset from an
/// alloca, the offset is negative, or the length is not a constant.
pub fn assignment_info(
    q: &Query,
    inst: InstId,
) -> Option<AssignmentInfo> {
    let instruction = q.function.instruction(inst);
    let (dest, size_in_bits) = match &instruction.opcode {
        Opcode::Store { .. } => {
            let value_type = q.type_of(*instruction.operands.first()?)?;
            (*instruction.operands.get(1)?, q.layout.size_bits(&q.context.types, value_type))
        }
        Opcode::Call(_) => {
            let callee = llrm_mir::memory::callee(q.context, q.function, inst)?;
            let id = q.globals.get(callee.0 as usize)?.name.intrinsic_id()?;
            use llrm_mir::valuetracking::pattern::IntrinsicId as I;
            if !matches!(id, I::Memset | I::Memcpy | I::MemcpyInline | I::Memmove) {
                return None;
            }
            let Some(Operand::Constant(length)) = instruction.operands.get(2).copied() else { return None };
            let ConstantKind::Int(bytes) = q.context.get(length).kind else { return None };
            (*instruction.operands.first()?, u64::try_from(bytes).ok()?.checked_mul(8)?)
        }
        _ => return None,
    };
    let width = match q.type_of(dest).map(|ty| q.context.types.get(ty)) {
        Some(Type::Pointer(space)) => q.layout.pointer(*space).index_bits,
        _ => 64,
    };
    let (base, offset) = strip_and_accumulate_constant_offsets(q, dest, width, true);
    let offset = i128::try_from(offset).ok()?;
    let offset = if width < 128 && offset >> (width - 1) & 1 == 1 { offset - (1_i128 << width) } else { offset };
    let offset_in_bytes = u64::try_from(offset).ok().filter(|&bytes| bytes != u64::MAX)?;
    let Operand::Value(base) = base else { return None };
    let ValueDef::Instruction(def) = q.function.value(base).def else { return None };
    matches!(q.function.instruction(def).opcode, Opcode::Alloca { .. }).then_some(AssignmentInfo {
        base,
        alloca: def,
        offset_in_bits: offset_in_bytes.checked_mul(8)?,
        size_in_bits,
    })
}

/// `DIExpression::calculateFragmentIntersect`: the bits of a variable's
/// fragment `var` (the memory at `dbg_ptr` plus `dbg_offset_in_bits`) that
/// the memory `[slice_offset, slice_offset + slice_size)` bits past
/// `slice_start` covers. `Some(None)` is the whole of `var`; None where the
/// distance of the pointers is no constant or the variable's size is not known.
pub fn calculate_fragment_intersect(
    q: &Query,
    slice_start: Operand,
    slice_offset_in_bits: u64,
    slice_size_in_bits: u64,
    dbg_ptr: Operand,
    dbg_offset_in_bits: i64,
    var: Fragment,
) -> Option<Option<Fragment>> {
    if var.size == 0 {
        return None;
    }
    let apart = get_pointer_offset_from(q, slice_start, dbg_ptr)?;
    let mem_start = apart * 8 + slice_offset_in_bits as i64 - dbg_offset_in_bits;
    let mem_end = mem_start + slice_size_in_bits as i64;
    if mem_end < 0 {
        return Some(Some(Fragment { offset: 0, size: 0 }));
    }
    let start_in_var = mem_start + var.offset as i64;
    let end_in_var = start_in_var + slice_size_in_bits as i64;
    // The slice of the variable, trimmed to `var` (`FragmentInfo::intersect`).
    let left = start_in_var.max(0).max(var.offset as i64);
    let right = end_in_var.min((var.offset + var.size) as i64);
    let trimmed = if right > left {
        Fragment { offset: left as u64, size: (right - left) as u64 }
    } else {
        Fragment { offset: 0, size: 0 }
    };
    Some((trimmed != var).then_some(trimmed))
}

/// `at::calculateFragmentIntersect`: as above, for the memory a store
/// writes and a `#dbg_assign`'s address and variable. None where the record
/// has no address, its address expression is more than an offset, or the
/// variable's size is not known.
pub fn calculate_fragment_intersect_of_assign(
    q: &Query,
    module: &Module,
    destination: Operand,
    slice_offset_in_bits: u64,
    slice_size_in_bits: u64,
    record: &DebugRecord,
) -> Option<Option<Fragment>> {
    let DebugWhat::Assign { address: Some(address), address_expression, .. } = record.what else { return None };
    let (offset_in_bytes, _) = extract_leading_offset(&module.expression(address_expression)?)?;
    let var = fragment_or_entire_variable(module, record)?;
    calculate_fragment_intersect(
        q,
        destination,
        slice_offset_in_bits,
        slice_size_in_bits,
        address,
        offset_in_bytes * 8,
        var,
    )
}

/// `DbgVariableRecord::getFragmentOrEntireVariable`: the fragment the record's
/// expression names, else all of the variable's type; None for a variable of
/// no known size.
pub fn fragment_or_entire_variable(
    module: &Module,
    record: &DebugRecord,
) -> Option<Fragment> {
    let ops = record.expression.and_then(|id| module.expression(id)).unwrap_or_default();
    if let Some(fragment) = fragment_of(&ops) {
        return Some(fragment);
    }
    Some(Fragment { offset: 0, size: variable_size_in_bits(module, record.variable)? })
}

/// The operand of the specialized node `id` written as `field:`.
fn field<'m>(
    module: &'m Module,
    id: MetadataId,
    name: &str,
) -> Option<&'m llrm_mir::module::MetadataOperand> {
    let special = module.special.get(&id)?;
    let at = special.fields.iter().position(|one| one == name)?;
    module.metadata[id.0 as usize].operands.get(at)
}

/// `DIVariable::getSizeInBits`: the variable's `size:`, else its type's, which
/// a typedef or qualifier takes from the type it wraps. None for size 0.
pub fn variable_size_in_bits(
    module: &Module,
    variable: MetadataId,
) -> Option<u64> {
    use llrm_mir::module::MetadataOperand as Operand;
    let size = |id| match field(module, id, "size")? {
        Operand::Constant(value) => match module.context.get(*value).kind {
            ConstantKind::Int(bits) => u64::try_from(bits).ok().filter(|&bits| bits != 0),
            _ => None,
        },
        _ => None,
    };
    if let Some(bits) = size(variable) {
        return Some(bits);
    }
    let Some(&Operand::Node(mut ty)) = field(module, variable, "type") else { return None };
    // A chain of base types is as long as the source's nesting.
    for _ in 0..module.metadata.len() {
        if let Some(bits) = size(ty) {
            return Some(bits);
        }
        let Some(&Operand::Node(base)) = field(module, ty, "baseType") else { return None };
        ty = base;
    }
    None
}

/// The `#dbg_assign` records linked to each `!DIAssignID`, by index in the
/// function's records (`at::getDVRAssignmentMarkers`, which the pin finds
/// through a map of the id's users): [`Function::assignment_markers`] scans
/// every record per call.
#[derive(Debug, Default)]
pub struct AssignmentMarkers {
    by_id: HashMap<MetadataId, Vec<usize>>,
}

impl AssignmentMarkers {
    pub fn new(function: &Function) -> AssignmentMarkers {
        let mut by_id: HashMap<MetadataId, Vec<usize>> = HashMap::new();
        for (at, record) in function.debug_records().iter().enumerate() {
            if let DebugWhat::Assign { id, .. } = record.what {
                by_id.entry(id).or_default().push(at);
            }
        }
        AssignmentMarkers { by_id }
    }

    /// The records linked to `inst` by the `!DIAssignID` it carries.
    pub fn of(
        &self,
        function: &Function,
        inst: InstId,
    ) -> &[usize] {
        function
            .instruction(inst)
            .metadata
            .iter()
            .find(|(kind, _)| kind == "DIAssignID")
            .and_then(|(_, id)| self.by_id.get(id))
            .map_or(&[], Vec::as_slice)
    }
}

/// The variables with a stack slot: those a `#dbg_assign` is linked to an
/// instruction of (`findVarsWithStackSlot`). An aggregate is the variable's
/// node alone: the inliner drops the callee's records, so none is inlined.
pub fn vars_with_stack_slot(
    function: &Function,
    markers: &AssignmentMarkers,
) -> HashSet<MetadataId> {
    function
        .walk()
        .flat_map(|(_, inst)| markers.of(function, inst))
        .map(|&at| function.debug_records()[at].variable)
        .collect()
}

/// What `buildOverlapMapAndRecordDeclares` finds out about a function.
#[derive(Debug, Default)]
pub struct Prepared {
    /// Per variable the variables of its aggregate whose bits it fully
    /// contains (`OverlapMap`).
    pub contains: HashMap<VariableId, Vec<VariableId>>,
    /// Per untagged store the variables it writes (`UntaggedStoreVars`).
    pub untagged_stores: HashMap<InstId, Vec<(VariableId, AssignmentInfo)>>,
    /// Per call that may write through a pointer to a tracked alloca, the
    /// variables of it, with the address and address expression of the
    /// record that names the alloca (`EscapingCallVars`).
    pub escaping_calls: HashMap<InstId, Vec<(VariableId, Operand, MetadataId)>>,
    /// One more than the last variable the dataflow tracks
    /// (`TrackedVariablesVectorSize`).
    pub tracked_variables: usize,
}

/// `fullyContains`: `a` has all the bits of `b`.
fn fully_contains(
    a: Fragment,
    b: Fragment,
) -> bool {
    b.offset >= a.offset && b.offset + b.size <= a.offset + a.size
}

/// The variable a record sets (`DebugVariable(DVR)`).
pub fn debug_variable_of(
    module: &Module,
    record: &DebugRecord,
) -> DebugVariable {
    let ops = record.expression.and_then(|id| module.expression(id)).unwrap_or_default();
    DebugVariable { variable: record.variable, fragment: fragment_of(&ops), inlined_at: None }
}

/// The escaping-call part of `buildOverlapMapAndRecordDeclares`: a call
/// that is no intrinsic, may write memory, and passes a pointer into a tracked
/// alloca the callee may write through (not `readonly`, `readnone` or
/// `byval`) may change the variables of that alloca, whole: which part is not
/// known.
#[allow(clippy::too_many_arguments)]
fn escaping_call(
    q: &Query,
    callees: &llrm_mir::memory::Callees,
    markers: &AssignmentMarkers,
    with_stack_slot: &HashSet<MetadataId>,
    builder: &mut FunctionVarLocsBuilder,
    inst: InstId,
    prepared: &mut Prepared,
) {
    use llrm_mir::valuetracking::pattern::{call_info, called_function};
    let function = q.function;
    let instruction = function.instruction(inst);
    let Some(info) = call_info(instruction) else { return };
    if let Some(callee) = llrm_mir::memory::callee(q.context, function, inst)
        && q.globals
            .get(callee.0 as usize)
            .is_some_and(|one| one.name.as_ref().is_some_and(|name| name.starts_with("llvm.")))
    {
        return;
    }
    if !llrm_mir::memory::of(q.context, callees, function, inst).writes {
        return;
    }
    let callee = called_function(q, instruction);
    let has_flag = |attrs: Option<&Vec<llrm_mir::Attribute>>, flag: &str| {
        attrs.is_some_and(|attrs| llrm_mir::memory::has(attrs, flag))
    };
    let mut seen: HashSet<VariableId> = HashSet::new();
    for (arg, &operand) in instruction.operands[..instruction.operands.len() - 1].iter().enumerate() {
        if !q.is_pointer(operand) {
            continue;
        }
        let site = info.argument_attrs.get(arg);
        let declared = callee.and_then(|callee| callee.parameter_attrs.get(arg));
        if [site, declared].into_iter().any(|attrs| has_flag(attrs, "readonly") || has_flag(attrs, "readnone")) {
            continue;
        }
        if site.is_some_and(|attrs| {
            attrs.iter().any(|one| matches!(one, llrm_mir::Attribute::Type(key, _) if key == "byval"))
        }) {
            continue;
        }
        let Operand::Value(object) = llrm_mir::valuetracking::underlying::get_underlying_object(q, operand, 6) else {
            continue;
        };
        let llrm_mir::ValueDef::Instruction(alloca) = function.value(object).def else { continue };
        if !matches!(function.instruction(alloca).opcode, Opcode::Alloca { .. }) {
            continue;
        }
        for &at in markers.of(function, alloca) {
            let record = &function.debug_records()[at];
            let DebugWhat::Assign { address: Some(address), address_expression, .. } = record.what else { continue };
            if !with_stack_slot.contains(&record.variable) {
                continue;
            }
            let variable =
                builder.insert_variable(DebugVariable { variable: record.variable, fragment: None, inlined_at: None });
            if seen.insert(variable) {
                prepared.escaping_calls.entry(inst).or_default().push((variable, address, address_expression));
            }
        }
    }
}

/// `buildOverlapMapAndRecordDeclares` without unknown stores (the IR has no
/// masked or vector store to make one): the variables the dataflow tracks,
/// which contain which, and which untagged stores write them; each
/// `#dbg_declare` becomes a location valid for the variable's whole scope.
///
/// O(n log n) in the records, for the fragments' sort, and quadratic in the
/// fragments of one variable, as the pin is. The pin numbers the variables
/// of its fragment map in the hash order of a `DenseMap`; here they follow
/// the order the records stand in, so the numbers (not the table) differ
/// from `llc -print-debug-ata`.
pub fn build_overlap_map(
    module: &Module,
    q: &Query,
    builder: &mut FunctionVarLocsBuilder,
    markers: &AssignmentMarkers,
    with_stack_slot: &HashSet<MetadataId>,
    callees: &llrm_mir::memory::Callees,
) -> Prepared {
    let function = q.function;
    let mut before: HashMap<InstId, Vec<usize>> = HashMap::new();
    for (at, record) in function.debug_records().iter().enumerate() {
        before.entry(record.before).or_default().push(at);
    }
    let mut prepared = Prepared::default();
    let mut seen: HashSet<DebugVariable> = HashSet::new();
    let mut fragments: BTreeMap<MetadataId, Vec<DebugVariable>> = BTreeMap::new();
    let mut declares = Vec::new();
    let mut note = |variable: DebugVariable| {
        if with_stack_slot.contains(&variable.variable) && seen.insert(variable) {
            fragments.entry(variable.variable).or_default().push(variable);
        }
    };
    for (_, inst) in function.walk() {
        for &at in before.get(&inst).map_or(&[][..], Vec::as_slice) {
            let record = &function.debug_records()[at];
            if matches!(record.what, DebugWhat::Declare(_)) {
                declares.push(at);
            } else {
                note(debug_variable_of(module, record));
            }
        }
        escaping_call(q, callees, markers, with_stack_slot, builder, inst, &mut prepared);
        let Some(info) = assignment_info(q, inst) else { continue };
        for &at in markers.of(function, info.alloca) {
            let record = &function.debug_records()[at];
            // Skip a record whose variable the bits written lie outside of.
            let Some(intersect) = calculate_fragment_intersect_of_assign(
                q,
                module,
                Operand::Value(info.base),
                info.offset_in_bits,
                info.size_in_bits,
                record,
            ) else {
                continue;
            };
            if intersect.is_some_and(|fragment| fragment.size == 0) {
                continue;
            }
            let own = debug_variable_of(module, record);
            let variable = DebugVariable { fragment: intersect.or(own.fragment), ..own };
            if !with_stack_slot.contains(&variable.variable) {
                continue;
            }
            // Only a store the ids do not tag is cached for later; a tagged
            // one is processed through its markers.
            if !function.instruction(inst).metadata.iter().any(|(kind, _)| kind == "DIAssignID") {
                prepared.untagged_stores.entry(inst).or_default().push((builder.insert_variable(variable), info));
            }
            note(variable);
        }
    }
    for list in fragments.values_mut() {
        list.sort_by_key(|one| one.fragment_or_default().size);
        for (at, one) in list.iter().enumerate() {
            let this = builder.insert_variable(*one);
            for other in &list[at + 1..] {
                if fully_contains(other.fragment_or_default(), one.fragment_or_default()) {
                    prepared.contains.entry(builder.insert_variable(*other)).or_default().push(this);
                }
            }
        }
    }
    prepared.tracked_variables = builder.number_of_variables() + 1;
    for at in declares {
        let record = &function.debug_records()[at];
        let DebugWhat::Declare(address) = record.what else { continue };
        let ops = record.expression.and_then(|id| module.expression(id)).unwrap_or_default();
        builder.add_single_loc_var(debug_variable_of(module, record), ops, record.location, vec![address]);
    }
    prepared
}

/// Blocks above which the analysis gives up (`debug-ata-max-blocks`).
pub const MAX_BLOCKS: usize = 10000;

/// `walkToAllocaAndPrependOffsetDeref`: the alloca `address` is a constant
/// in-bounds offset from, and `ops` as the location of the variable that is
/// in the memory there: the offset ahead of them and a `deref` at their end,
/// before a fragment.
fn alloca_and_deref(
    q: &Query,
    address: Operand,
    mut ops: Vec<DwOp>,
) -> (Operand, Vec<DwOp>) {
    let width = match q.type_of(address).map(|ty| q.context.types.get(ty)) {
        Some(Type::Pointer(space)) => q.layout.pointer(*space).index_bits,
        _ => 64,
    };
    let (base, offset) = strip_and_accumulate_constant_offsets(q, address, width, false);
    if offset != 0 {
        ops.insert(0, DwOp::PlusUconst(offset as u64));
    }
    let at = ops.iter().position(|op| matches!(op, DwOp::Fragment { .. })).unwrap_or(ops.len());
    ops.insert(at, DwOp::Deref);
    (base, ops)
}

/// The dataflow's state and its transfer functions
/// (`AssignmentTrackingLowering`).
pub struct Lowering<'a> {
    module: &'a Module,
    q: &'a Query<'a>,
    markers: &'a AssignmentMarkers,
    with_slot: &'a HashSet<MetadataId>,
    prepared: Prepared,
    builder: FunctionVarLocsBuilder,
    /// The records standing before each instruction, in order.
    records: HashMap<InstId, Vec<usize>>,
    /// The locations to start, by point (`InsertBeforeMap`).
    insert_before: BTreeMap<InsertPt, Vec<VarLocInfo>>,
    /// The variables whose live kind was set in this frame.
    touched: Vec<VariableId>,
    /// Aggregates that are not in their stack slot all the time
    /// (`NotAlwaysStackHomed`).
    not_always_stack_homed: HashSet<MetadataId>,
}

impl<'a> Lowering<'a> {
    pub fn new(
        module: &'a Module,
        q: &'a Query<'a>,
        markers: &'a AssignmentMarkers,
        with_slot: &'a HashSet<MetadataId>,
        prepared: Prepared,
        builder: FunctionVarLocsBuilder,
    ) -> Lowering<'a> {
        let mut records: HashMap<InstId, Vec<usize>> = HashMap::new();
        for (at, record) in q.function.debug_records().iter().enumerate() {
            records.entry(record.before).or_default().push(at);
        }
        Lowering {
            module,
            q,
            markers,
            with_slot,
            prepared,
            builder,
            records,
            insert_before: BTreeMap::new(),
            touched: Vec::new(),
            not_always_stack_homed: HashSet::new(),
        }
    }

    fn ops_of(
        &self,
        expression: Option<MetadataId>,
    ) -> Vec<DwOp> {
        expression.and_then(|id| self.module.expression(id)).unwrap_or_default()
    }

    fn variable_of(
        &mut self,
        record: &DebugRecord,
    ) -> VariableId {
        self.builder.insert_variable(debug_variable_of(self.module, record))
    }

    fn contained(
        &self,
        variable: VariableId,
    ) -> Vec<VariableId> {
        self.prepared.contains.get(&variable).cloned().unwrap_or_default()
    }

    fn set_loc_kind(
        &mut self,
        live: &mut BlockInfo,
        variable: VariableId,
        kind: LocKind,
    ) {
        for one in std::iter::once(variable).chain(self.contained(variable)) {
            live.set_loc_kind(one, kind);
            self.touched.push(one);
        }
    }

    /// `addMemDef`, and `addDbgDef` for `AssignmentKind::Debug`: the
    /// assignment is the variable's, and its fragments' without a source.
    fn add_def(
        &self,
        live: &mut BlockInfo,
        kind: AssignmentKind,
        variable: VariableId,
        assignment: Assignment,
    ) {
        live.set_assignment(kind, variable, assignment);
        let fragment = match assignment {
            Assignment::Known { id, .. } => Assignment::Known { id, source: None },
            Assignment::NoneOrPhi => Assignment::NoneOrPhi,
        };
        for one in self.contained(variable) {
            live.set_assignment(kind, one, fragment);
        }
    }

    /// `hasVarWithAssignment`: the variable and its fragments all have it.
    fn has_assignment(
        &self,
        live: &BlockInfo,
        kind: AssignmentKind,
        variable: VariableId,
        assignment: &Assignment,
    ) -> bool {
        live.has_assignment(kind, variable, assignment)
            && self.contained(variable).into_iter().all(|one| live.has_assignment(kind, one, assignment))
    }

    fn emit(
        &mut self,
        at: InsertPt,
        variable: VariableId,
        expression: Vec<DwOp>,
        location: Option<MetadataId>,
        values: Vec<Operand>,
    ) {
        self.insert_before.entry(at).or_default().push(VarLocInfo { variable, expression, location, values });
    }

    /// `emitDbgValue`: a location for what `source` says, of `kind`, at `at`.
    fn emit_dbg_value(
        &mut self,
        mut kind: LocKind,
        source: &DebugRecord,
        at: InsertPt,
    ) {
        let variable = self.variable_of(source);
        let own = self.ops_of(source.expression);
        if kind == LocKind::Mem {
            let DebugWhat::Assign { address, address_expression, .. } = source.what else {
                panic!("a memory location is of a #dbg_assign")
            };
            match address {
                // The address was dropped: a value location.
                None => kind = LocKind::Val,
                Some(address) => {
                    let mut ops = self.ops_of(Some(address_expression));
                    if let Some(fragment) = fragment_of(&own) {
                        ops.push(DwOp::Fragment { offset: fragment.offset, size: fragment.size });
                    }
                    let (base, ops) = alloca_and_deref(self.q, address, ops);
                    self.emit(at, variable, ops, source.location, vec![base]);
                    return;
                }
            }
        }
        let values = match (kind, source.what) {
            (LocKind::None, _) => Vec::new(),
            (_, DebugWhat::Value(value) | DebugWhat::Piece { value, .. }) => vec![value],
            (_, DebugWhat::Assign { value, .. }) => value.into_iter().collect(),
            (_, DebugWhat::Args(list)) => self.q.function.debug_args(list).to_vec(),
            _ => Vec::new(),
        };
        self.emit(at, variable, own, source.location, values);
    }

    /// `processTaggedInstruction`: a store linked to `#dbg_assign`s.
    fn process_tagged(
        &mut self,
        live: &mut BlockInfo,
        inst: InstId,
        after: InsertPt,
    ) {
        let function = self.q.function;
        for &at in self.markers.of(function, inst) {
            let assign = function.debug_records()[at];
            let DebugWhat::Assign { id, .. } = assign.what else { continue };
            let variable = self.variable_of(&assign);
            let stored = Assignment::Known { id, source: None };
            self.add_def(live, AssignmentKind::Stack, variable, stored);
            if self.has_assignment(live, AssignmentKind::Debug, variable, &stored) {
                self.set_loc_kind(live, variable, LocKind::Mem);
                self.emit_dbg_value(LocKind::Mem, &assign, after);
                return;
            }
            match live.loc_kind(variable) {
                LocKind::Val => self.set_loc_kind(live, variable, LocKind::Val),
                LocKind::Mem => match *live.assignment(AssignmentKind::Debug, variable) {
                    Assignment::NoneOrPhi => {
                        self.set_loc_kind(live, variable, LocKind::None);
                        self.emit_dbg_value(LocKind::None, &assign, after);
                    }
                    Assignment::Known { source, .. } => {
                        self.set_loc_kind(live, variable, LocKind::Val);
                        match source {
                            Some(source) => self.emit_dbg_value(LocKind::Val, &source, after),
                            None => self.emit_dbg_value(LocKind::None, &assign, after),
                        }
                    }
                },
                LocKind::None => self.set_loc_kind(live, variable, LocKind::None),
            }
        }
    }

    /// `processUntaggedInstruction`: a store the ids do not tag is taken to
    /// write the stack home of the variables it lies in.
    fn process_untagged(
        &mut self,
        live: &mut BlockInfo,
        inst: InstId,
        after: InsertPt,
    ) {
        let Some(variables) = self.prepared.untagged_stores.get(&inst).cloned() else { return };
        for (variable, info) in variables {
            self.add_def(live, AssignmentKind::Stack, variable, Assignment::NoneOrPhi);
            self.add_def(live, AssignmentKind::Debug, variable, Assignment::NoneOrPhi);
            self.set_loc_kind(live, variable, LocKind::Mem);
            let mut ops = Vec::new();
            if info.offset_in_bits != 0 {
                ops.push(DwOp::PlusUconst(info.offset_in_bits / 8));
            }
            ops.push(DwOp::Deref);
            if let Some(fragment) = self.builder.variable(variable).fragment {
                ops.push(DwOp::Fragment { offset: fragment.offset, size: fragment.size });
            }
            self.emit(after, variable, ops, None, vec![Operand::Value(info.base)]);
        }
    }

    /// `processEscapingCall`: a call that may write a tracked variable is
    /// taken to leave the value now in memory as the variable's.
    fn process_escaping_call(
        &mut self,
        live: &mut BlockInfo,
        inst: InstId,
        after: InsertPt,
    ) {
        let Some(variables) = self.prepared.escaping_calls.get(&inst).cloned() else { return };
        for (variable, address, address_expression) in variables {
            self.add_def(live, AssignmentKind::Stack, variable, Assignment::NoneOrPhi);
            self.add_def(live, AssignmentKind::Debug, variable, Assignment::NoneOrPhi);
            self.set_loc_kind(live, variable, LocKind::Mem);
            let mut ops = self.ops_of(Some(address_expression));
            if let Some(fragment) = self.builder.variable(variable).fragment {
                ops.push(DwOp::Fragment { offset: fragment.offset, size: fragment.size });
            }
            let (base, ops) = alloca_and_deref(self.q, address, ops);
            self.emit(after, variable, ops, None, vec![base]);
        }
    }

    /// `processDbgAssign`.
    fn process_dbg_assign(
        &mut self,
        live: &mut BlockInfo,
        record: &DebugRecord,
        after: InsertPt,
    ) {
        if !self.with_slot.contains(&record.variable) {
            return;
        }
        let DebugWhat::Assign { id, address, .. } = record.what else { return };
        let variable = self.variable_of(record);
        let assignment = Assignment::Known { id, source: Some(*record) };
        self.add_def(live, AssignmentKind::Debug, variable, assignment);
        let kind = if self.has_assignment(live, AssignmentKind::Stack, variable, &assignment) {
            if address.is_none() { LocKind::Val } else { LocKind::Mem }
        } else {
            LocKind::Val
        };
        self.set_loc_kind(live, variable, kind);
        self.emit_dbg_value(kind, record, after);
    }

    /// `processDbgValue`: a `#dbg_value` overrides, with no assignment.
    fn process_dbg_value(
        &mut self,
        live: &mut BlockInfo,
        record: &DebugRecord,
        after: InsertPt,
    ) {
        if !self.with_slot.contains(&record.variable) {
            return;
        }
        let variable = self.variable_of(record);
        self.add_def(live, AssignmentKind::Debug, variable, Assignment::NoneOrPhi);
        self.set_loc_kind(live, variable, LocKind::Val);
        self.emit_dbg_value(LocKind::Val, record, after);
    }

    /// `process`: the instructions of `block` in order, each after the
    /// records standing before it; `live` is the state going in and, after,
    /// the state going out.
    pub fn process(
        &mut self,
        block: llrm_mir::BlockId,
        live: &mut BlockInfo,
    ) {
        let function = self.q.function;
        let instructions: Vec<InstId> = function.iter_block(block).collect();
        // A frame is an instruction and the records standing before the next;
        // the records before the first one are a frame of their own.
        if let Some(&first) = instructions.first() {
            self.process_records(live, first);
            self.finish_frame(live);
        }
        for (at, &inst) in instructions.iter().enumerate() {
            if function.instruction(inst).opcode.is_terminator() {
                break;
            }
            let next = instructions[at + 1];
            let after = (next, 0);
            self.insert_before.remove(&after);
            if function.instruction(inst).metadata.iter().any(|(kind, _)| kind == "DIAssignID") {
                self.process_tagged(live, inst, after);
            } else {
                self.process_untagged(live, inst, after);
            }
            self.process_escaping_call(live, inst, after);
            self.process_records(live, next);
            self.finish_frame(live);
        }
    }

    /// The records standing before `inst`, in order.
    fn process_records(
        &mut self,
        live: &mut BlockInfo,
        inst: InstId,
    ) {
        let function = self.q.function;
        let records = self.records.get(&inst).cloned().unwrap_or_default();
        for (slot, &index) in records.iter().enumerate() {
            let record = function.debug_records()[index];
            let after = (inst, slot as u32 + 1);
            // The location a record made before is made again.
            self.insert_before.remove(&after);
            if matches!(record.what, DebugWhat::Declare(_)) {
                continue;
            }
            if fragment_of(&self.ops_of(record.expression)).is_some_and(|fragment| fragment.size == 0) {
                continue;
            }
            match record.what {
                DebugWhat::Assign { .. } => self.process_dbg_assign(live, &record, after),
                _ => self.process_dbg_value(live, &record, after),
            }
        }
    }

    /// `run`: the dataflow over the function's blocks, two worklists in
    /// reverse post order as the pin has them, then the locations as the
    /// table: a variable always in its stack home has one location for its
    /// whole scope, the others their wedges, and the variables with no stack
    /// slot their records as they stand (`emitPromotedVarLocs`).
    ///
    /// Complexity: the pin's: a block is processed again when its entry
    /// state changes, a state only moves up its lattice (a variable's kind
    /// and assignments), so O(blocks x variables) passes of O(instructions),
    /// cut off at [`MAX_BLOCKS`] blocks.
    pub fn run(mut self) -> Option<(FunctionVarLocsBuilder, bool)> {
        use std::collections::BTreeSet;
        let function = self.q.function;
        if function.layout().len() > MAX_BLOCKS {
            return None;
        }
        let size = self.prepared.tracked_variables;
        let order = llrm_mir::blockorder::reverse_post_order_traversal(function);
        let number: HashMap<llrm_mir::BlockId, usize> = order.iter().enumerate().map(|(at, &b)| (b, at)).collect();
        let mut live_in: HashMap<llrm_mir::BlockId, BlockInfo> = HashMap::new();
        let mut live_out: HashMap<llrm_mir::BlockId, BlockInfo> = HashMap::new();
        let mut visited: HashSet<llrm_mir::BlockId> = HashSet::new();
        let mut worklist: BTreeSet<usize> = (0..order.len()).collect();
        let mut pending: BTreeSet<usize> = BTreeSet::new();
        while !worklist.is_empty() {
            while let Some(at) = worklist.pop_first() {
                let block = order[at];
                let mut changed = self.join(block, &visited, size, &mut live_in, &live_out);
                changed |= visited.insert(block);
                if !changed {
                    continue;
                }
                let mut live = live_in[&block].clone();
                self.process(block, &mut live);
                if live_out.get(&block) != Some(&live) {
                    live_out.insert(block, live);
                    for successor in function.successors(block).iter() {
                        if let Some(&n) = number.get(successor) {
                            pending.insert(n);
                        }
                    }
                }
            }
            std::mem::swap(&mut worklist, &mut pending);
        }
        Some(self.finish())
    }

    /// `join`: the state at the top of `block` from its predecessors visited
    /// so far (a back edge not yet visited counts as nothing); whether it
    /// changed.
    fn join(
        &self,
        block: llrm_mir::BlockId,
        visited: &HashSet<llrm_mir::BlockId>,
        size: usize,
        live_in: &mut HashMap<llrm_mir::BlockId, BlockInfo>,
        live_out: &HashMap<llrm_mir::BlockId, BlockInfo>,
    ) -> bool {
        let preds: Vec<_> =
            self.q.function.predecessors(block).iter().copied().filter(|p| visited.contains(p)).collect();
        let joined = match preds.split_first() {
            None => {
                return if live_in.contains_key(&block) {
                    false
                } else {
                    live_in.insert(block, BlockInfo::new(size));
                    true
                };
            }
            Some((first, [])) => live_out[first].clone(),
            Some((first, rest)) => {
                rest.iter().fold(live_out[first].clone(), |acc, p| BlockInfo::join(&acc, &live_out[p], size))
            }
        };
        if live_in.get(&block) == Some(&joined) {
            return false;
        }
        live_in.insert(block, joined);
        true
    }

    /// The table from what the dataflow made.
    fn finish(mut self) -> (FunctionVarLocsBuilder, bool) {
        let mut inserted = false;
        let mut always: HashSet<MetadataId> = HashSet::new();
        for locs in self.insert_before.values() {
            for loc in locs {
                let aggregate = self.builder.variable(loc.variable).variable;
                if self.not_always_stack_homed.contains(&aggregate) {
                    continue;
                }
                // Different fragments in different allocas, or anything but
                // a bare deref, fall back to wedges.
                if loc.expression != [DwOp::Deref] {
                    self.not_always_stack_homed.insert(aggregate);
                    continue;
                }
                if always.insert(aggregate) {
                    inserted = true;
                    let variable = *self.builder.variable(loc.variable);
                    self.builder.add_single_loc_var(variable, Vec::new(), loc.location, loc.values.clone());
                }
            }
        }
        let wedges = std::mem::take(&mut self.insert_before);
        for (at, locs) in wedges {
            let kept: Vec<VarLocInfo> = locs
                .into_iter()
                .filter(|loc| !always.contains(&self.builder.variable(loc.variable).variable))
                .collect();
            inserted |= !kept.is_empty();
            self.builder.set_wedge(at, kept);
        }
        inserted |= self.emit_promoted();
        (self.builder, inserted)
    }

    /// `emitPromotedVarLocs`: a record of a variable with no stack slot is
    /// a location where it stands.
    fn emit_promoted(&mut self) -> bool {
        let mut any = false;
        let function = self.q.function;
        for record in function.debug_records() {
            if matches!(record.what, DebugWhat::Declare(_)) || self.with_slot.contains(&record.variable) {
                continue;
            }
            let variable = debug_variable_of(self.module, record);
            let ops = self.ops_of(record.expression);
            let values = match record.what {
                DebugWhat::Value(value) | DebugWhat::Piece { value, .. } => vec![value],
                DebugWhat::Assign { value, .. } => value.into_iter().collect(),
                DebugWhat::Args(list) => function.debug_args(list).to_vec(),
                _ => Vec::new(),
            };
            let slot = self
                .records
                .get(&record.before)
                .and_then(|list| list.iter().position(|&at| std::ptr::eq(&function.debug_records()[at], record)));
            let at = (record.before, slot.map_or(0, |one| one as u32));
            self.builder.add_var_loc(at, variable, ops, record.location, values);
            any = true;
        }
        any
    }

    /// The end of a frame: a variable that is not in memory now cannot be
    /// a single location.
    fn finish_frame(
        &mut self,
        live: &BlockInfo,
    ) {
        for variable in std::mem::take(&mut self.touched) {
            if live.loc_kind(variable) != LocKind::Mem {
                self.not_always_stack_homed.insert(self.builder.variable(variable).variable);
            }
        }
    }
}

/// `analyzeFunction`: the locations of `function`'s variables. `coalesce`
/// is whether adjacent fragments in memory are stated as one
/// (`shouldCoalesceFragments`: the target uses instruction referencing).
pub fn analyze(
    module: &Module,
    q: &Query,
    coalesce: bool,
) -> Option<FunctionVarLocs> {
    let function = q.function;
    let markers = AssignmentMarkers::new(function);
    let with_slot = vars_with_stack_slot(function, &markers);
    let mut builder = FunctionVarLocsBuilder::default();
    let callees = llrm_mir::memory::callees(module);
    let prepared = build_overlap_map(module, q, &mut builder, &markers, &with_slot, &callees);
    let (mut builder, changed) = Lowering::new(module, q, &markers, &with_slot, prepared, builder).run()?;
    if changed {
        let mut count: HashMap<InstId, u32> = HashMap::new();
        for record in function.debug_records() {
            *count.entry(record.before).or_default() += 1;
        }
        let slots = |inst: InstId| count.get(&inst).copied().unwrap_or(0);
        fill::run(module, function, &mut builder, &with_slot, &slots, coalesce);
        redundant::run(module, function, &mut builder, &slots);
    }
    Some(builder.finish())
}

/// `AssignmentTrackingPass` over a module: each function with
/// `#dbg_assign` records has them replaced by the records its analysis
/// states. O(n) in the module's instructions and records, plus the
/// dataflow's own bound per function (`MAX_BLOCKS`).
pub fn module_rewritten(module: &mut Module) -> Result<(), String> {
    let Some(text) = module.datalayout.clone() else { return Ok(()) };
    let layout = llrm_mir::datalayout::DataLayout::parse(&text)?;
    let wanted: Vec<llrm_mir::GlobalId> = module
        .functions()
        .filter(|(_, _, function)| {
            function.debug_records().iter().any(|one| matches!(one.what, llrm_mir::DebugWhat::Assign { .. }))
        })
        .map(|(id, ..)| id)
        .collect();
    for id in wanted {
        let locs = {
            let function = module.global(id).function().expect("a function");
            analyze(module, &Query::new(module, function, &layout), false)
        };
        if let Some(locs) = locs {
            rewrite(module, id, &locs);
        }
    }
    Ok(())
}

/// The operations of an expression as LLVM prints them.
pub fn expression_text(ops: &[DwOp]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for op in ops {
        match op_words(op) {
            Some((name, numbers)) => {
                parts.push(name.to_owned());
                parts.extend(numbers.iter().map(i128::to_string));
            }
            None => parts.push("DW_OP_?".to_owned()),
        }
    }
    parts.join(", ")
}

/// The words an operation is written with: its name and numbers. `None` for
/// [`DwOp::Other`], which keeps neither.
fn op_words(op: &DwOp) -> Option<(&'static str, Vec<i128>)> {
    use llrm_mir::module::{DwBinary, DwUnary};
    Some(match *op {
        DwOp::PlusUconst(n) => ("DW_OP_plus_uconst", vec![n.into()]),
        DwOp::StackValue => ("DW_OP_stack_value", vec![]),
        DwOp::Fragment { offset, size } => ("DW_OP_LLVM_fragment", vec![offset.into(), size.into()]),
        DwOp::Arg(n) => ("DW_OP_LLVM_arg", vec![n.into()]),
        DwOp::Constu(n) => ("DW_OP_constu", vec![n.into()]),
        DwOp::Consts(n) => ("DW_OP_consts", vec![n.into()]),
        DwOp::Deref => ("DW_OP_deref", vec![]),
        DwOp::Binary(one) => (
            match one {
                DwBinary::Plus => "DW_OP_plus",
                DwBinary::Minus => "DW_OP_minus",
                DwBinary::Mul => "DW_OP_mul",
                DwBinary::And => "DW_OP_and",
                DwBinary::Or => "DW_OP_or",
                DwBinary::Xor => "DW_OP_xor",
                DwBinary::Shl => "DW_OP_shl",
                DwBinary::Shr => "DW_OP_shr",
                DwBinary::Shra => "DW_OP_shra",
            },
            vec![],
        ),
        DwOp::Unary(one) => (
            match one {
                DwUnary::Not => "DW_OP_not",
                DwUnary::Neg => "DW_OP_neg",
            },
            vec![],
        ),
        DwOp::Other => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variable(node: u32) -> DebugVariable {
        DebugVariable { variable: MetadataId(node), fragment: None, inlined_at: None }
    }

    fn known(id: u32) -> Assignment {
        Assignment::Known { id: MetadataId(id), source: None }
    }

    /// Different kinds of location join to `None`; equal ones to themselves.
    #[test]
    fn locations_of_two_kinds_join_to_none() {
        use LocKind::*;
        assert_eq!(
            (Mem.join(Mem), Val.join(Val), Mem.join(Val), None.join(Mem), None.join(None)),
            (Mem, Val, None, None, None)
        );
    }

    /// Two paths that made the same assignment keep it; different ones, or an
    /// unknown one, make a phi.
    #[test]
    fn assignments_join_to_the_same_one_or_a_phi() {
        assert!(known(1).join(&known(1)).is_same_source_assignment(&known(1)));
        assert!(known(1).join(&known(2)).is_same_source_assignment(&Assignment::NoneOrPhi));
        assert!(known(1).join(&Assignment::NoneOrPhi).is_same_source_assignment(&Assignment::NoneOrPhi));
        assert!(Assignment::NoneOrPhi.join(&Assignment::NoneOrPhi).is_same_source_assignment(&Assignment::NoneOrPhi));
    }

    /// A variable only one path tracked stays as that path had it; one both
    /// track is joined.
    #[test]
    fn a_join_tracks_what_either_path_does_and_joins_what_both_do() {
        let (x, y) = (VariableId(1), VariableId(2));
        let mut a = BlockInfo::new(3);
        let mut b = BlockInfo::new(3);
        a.set_loc_kind(x, LocKind::Mem);
        a.set_assignment(AssignmentKind::Stack, x, known(7));
        b.set_loc_kind(x, LocKind::Val);
        b.set_assignment(AssignmentKind::Stack, x, known(7));
        a.set_loc_kind(y, LocKind::Val);
        let join = BlockInfo::join(&a, &b, 3);
        assert_eq!(join.loc_kind(x), LocKind::None, "Mem and Val");
        assert!(join.has_assignment(AssignmentKind::Stack, x, &known(7)), "the same assignment on both");
        assert!(join.is_variable_tracked(y) && !join.is_variable_tracked(VariableId(0)));
        assert_eq!(join.loc_kind(y), LocKind::None, "the join does not copy what only one path knew");
    }

    /// Two blocks are equal when their tracked variables are: what an
    /// untracked slot holds does not count.
    #[test]
    fn block_infos_compare_on_the_variables_they_track() {
        let x = VariableId(1);
        let (mut a, mut b) = (BlockInfo::new(2), BlockInfo::new(2));
        assert_eq!(a, b);
        a.set_loc_kind(x, LocKind::Mem);
        assert_ne!(a, b);
        b.set_loc_kind(x, LocKind::Mem);
        assert_eq!(a, b);
        a.set_assignment(AssignmentKind::Debug, x, known(1));
        b.set_assignment(AssignmentKind::Debug, x, known(2));
        assert_ne!(a, b);
    }

    /// A variable has one id however often it is asked for, and the ids start
    /// at 1.
    #[test]
    fn a_variable_has_one_id_and_ids_start_at_one() {
        let mut builder = FunctionVarLocsBuilder::default();
        let x = builder.insert_variable(variable(5));
        let y = builder.insert_variable(variable(6));
        assert_eq!((x, y, builder.insert_variable(variable(5))), (VariableId(1), VariableId(2), VariableId(1)));
        let piece = DebugVariable { fragment: Some(Fragment { offset: 0, size: 8 }), ..variable(5) };
        assert_eq!(builder.insert_variable(piece), VariableId(3), "a fragment is another variable");
        assert_eq!(builder.number_of_variables(), 3);
    }

    /// Locations stand before their instruction in the order they were added,
    /// and single locations apart from them.
    #[test]
    fn locations_keep_their_instruction_and_order() {
        let mut builder = FunctionVarLocsBuilder::default();
        builder.add_var_loc((InstId(3), 0), variable(5), vec![], None, vec![]);
        builder.add_var_loc((InstId(3), 0), variable(6), vec![DwOp::Deref], None, vec![]);
        builder.add_single_loc_var(variable(7), vec![DwOp::Deref], None, vec![]);
        let result = builder.finish();
        let names: Vec<u32> = result.locs_before(InstId(3)).iter().map(|one| one.variable.0).collect();
        assert_eq!(names, [1, 2]);
        assert!(result.locs_before(InstId(4)).is_empty());
        assert_eq!(result.single_locs().len(), 1);
        assert_eq!(result.variable(VariableId(3)).variable, MetadataId(7));
    }

    const DATALAYOUT: &str = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128";

    /// `int a[3]` with its first word, and the second one, stored: clang's
    /// own records, `tag` the id of the first store.
    fn array(
        first: &str,
        second: &str,
    ) -> Module {
        array_calling(first, second, "")
    }

    /// As [`array`], with `call` (a line of the block) before the `ret`.
    fn array_calling(
        first: &str,
        second: &str,
        call: &str,
    ) -> Module {
        let text = format!(
            "target datalayout = \"{DATALAYOUT}\"\n\
define i32 @f(i32 %n) {{\nentry:\n\
  %a = alloca [3 x i32], align 4, !DIAssignID !24\n\
    #dbg_assign(i1 poison, !20, !DIExpression(), !24, ptr %a, !DIExpression(), !25)\n\
  store i32 %n, ptr %a, align 4{first}\n\
    #dbg_assign(i32 %n, !20, !DIExpression(DW_OP_LLVM_fragment, 0, 32), !28, ptr %a, !DIExpression(), !25)\n\
  %p = getelementptr inbounds i8, ptr %a, i64 4\n\
  store i32 %n, ptr %p, align 4{second}\n\
    #dbg_assign(i32 %n, !20, !DIExpression(DW_OP_LLVM_fragment, 32, 32), !29, ptr %a, !DIExpression(DW_OP_plus_uconst, 4), !25)\n\
  {call}\n  ret i32 %n\n}}\n\
declare void @g(ptr)\ndeclare void @h(ptr readonly)\n\
!17 = !DIBasicType(name: \"int\", size: 32, encoding: DW_ATE_signed)\n\
!20 = !DILocalVariable(name: \"a\", scope: !14, type: !21)\n\
!21 = !DICompositeType(tag: DW_TAG_array_type, baseType: !17, size: 96)\n\
!24 = distinct !DIAssignID()\n!28 = distinct !DIAssignID()\n!29 = distinct !DIAssignID()\n\
!14 = distinct !DISubprogram(name: \"f\")\n!25 = !DILocation(line: 0, scope: !14)\n"
        );
        llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"))
    }

    fn function_f(module: &Module) -> &Function {
        module.global(module.named("f").expect("@f")).function().expect("a body")
    }

    /// The layout the function's pointers are read with.
    fn prepare(module: &Module) -> (Prepared, FunctionVarLocsBuilder) {
        let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
        let function = function_f(module);
        let q = Query::new(module, function, &layout);
        let markers = AssignmentMarkers::new(function);
        let with_slot = vars_with_stack_slot(function, &markers);
        let mut builder = FunctionVarLocsBuilder::default();
        let prepared =
            build_overlap_map(module, &q, &mut builder, &markers, &with_slot, &llrm_mir::memory::callees(module));
        (prepared, builder)
    }

    const TAGGED_1: &str = ", !DIAssignID !28";
    const TAGGED_2: &str = ", !DIAssignID !29";

    /// The leading offset stops at a deref or a fragment, reads
    /// `constu n; plus|minus`, and refuses anything else: a wrong read put a
    /// store's bits at the wrong place in its variable.
    #[test]
    fn the_leading_offset_is_what_the_pin_extracts() {
        use llrm_mir::module::DwBinary::{Minus, Plus};
        let frag = DwOp::Fragment { offset: 0, size: 8 };
        assert_eq!(extract_leading_offset(&[]), Some((0, &[][..])));
        assert_eq!(extract_leading_offset(&[DwOp::PlusUconst(4), DwOp::Deref]), Some((4, &[DwOp::Deref][..])));
        assert_eq!(
            extract_leading_offset(&[DwOp::Constu(8), DwOp::Binary(Minus), DwOp::PlusUconst(3), frag]),
            Some((-5, &[frag][..]))
        );
        assert_eq!(extract_leading_offset(&[DwOp::Constu(8), DwOp::Binary(Plus)]), Some((8, &[][..])));
        assert_eq!(extract_leading_offset(&[DwOp::Constu(8), DwOp::Deref]), None);
        assert_eq!(extract_leading_offset(&[DwOp::StackValue]), None);
        assert_eq!(extract_leading_offset(&[DwOp::Arg(1)]), None);
    }

    /// A store of an `i32` 4 bytes into a 12-byte alloca writes bits 32..64
    /// of it; one through a pointer that is no constant offset writes none.
    #[test]
    fn a_store_writes_bits_of_its_alloca() {
        let module = array(TAGGED_1, TAGGED_2);
        let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
        let function = function_f(&module);
        let q = Query::new(&module, function, &layout);
        let stores: Vec<_> = function
            .walk()
            .filter(|&(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Store { .. }))
            .map(|(_, inst)| assignment_info(&q, inst).map(|one| (one.offset_in_bits, one.size_in_bits)))
            .collect();
        assert_eq!(stores, [Some((0, 32)), Some((32, 32))]);
    }

    /// The variable `a` has no fragment of its own, so its size is its
    /// type's; the two halves clang wrote are inside it, and neither
    /// contains the other.
    #[test]
    fn a_variable_contains_the_fragments_written_to_it() {
        let module = array(TAGGED_1, TAGGED_2);
        let (prepared, builder) = prepare(&module);
        let frag = |at: usize| builder.variable(VariableId(at as u32)).fragment;
        assert_eq!(builder.number_of_variables(), 3, "a, a[0..32), a[32..64)");
        assert_eq!(prepared.tracked_variables, 4);
        let whole = (1..=3).find(|&at| frag(at).is_none()).expect("the whole variable");
        let mut inside: Vec<_> =
            prepared.contains[&VariableId(whole as u32)].iter().map(|id| frag(id.0 as usize)).collect();
        inside.sort_by_key(|one| one.map(|f| f.offset));
        assert_eq!(inside, [Some(Fragment { offset: 0, size: 32 }), Some(Fragment { offset: 32, size: 32 })]);
        assert!(prepared.contains.len() == 1, "the halves hold nothing");
    }

    /// A store that lost its `!DIAssignID` is recorded with the variable
    /// bits it wrote (the pin's `UntaggedStoreVars`); a tagged one is not:
    /// treating a tagged store as untagged double counted its assignment.
    #[test]
    fn only_a_store_without_an_id_is_an_untagged_store() {
        let module = array(TAGGED_1, "");
        let (prepared, builder) = prepare(&module);
        let function = function_f(&module);
        let untagged: Vec<_> = prepared
            .untagged_stores
            .values()
            .flatten()
            .map(|(variable, info)| (builder.variable(*variable).fragment, info.offset_in_bits))
            .collect();
        assert_eq!(untagged, [(Some(Fragment { offset: 32, size: 32 }), 32)]);
        assert_eq!(prepared.untagged_stores.len(), 1);
        let _ = function;
    }

    /// The bits of a variable a store covers, past a `dbg_assign`'s own
    /// offset: a 4-byte store at +4 of an address that is itself at +4 is the
    /// variable's first word.
    #[test]
    fn a_fragment_is_found_relative_to_the_records_address() {
        let module = array(TAGGED_1, TAGGED_2);
        let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
        let function = function_f(&module);
        let q = Query::new(&module, function, &layout);
        let (_, alloca) = function.walk().next().map(|(b, i)| (b, function.instruction(i).result.unwrap())).unwrap();
        let slice = |offset, size, dbg_offset, var| {
            calculate_fragment_intersect(
                &q,
                Operand::Value(alloca),
                offset,
                size,
                Operand::Value(alloca),
                dbg_offset,
                var,
            )
        };
        let whole = Fragment { offset: 0, size: 96 };
        assert_eq!(slice(32, 32, 0, whole), Some(Some(Fragment { offset: 32, size: 32 })));
        assert_eq!(slice(0, 96, 0, whole), Some(None), "all of the variable");
        assert_eq!(slice(32, 32, 32, whole), Some(Some(Fragment { offset: 0, size: 32 })));
        assert_eq!(slice(0, 32, 64, whole), Some(Some(Fragment { offset: 0, size: 0 })), "before the variable");
        assert_eq!(slice(0, 32, 0, Fragment { offset: 0, size: 0 }), None, "no size");
    }

    /// Runs the dataflow over the entry block and gives the locations it made,
    /// as (instruction's name or opcode, slot, variable's fragment, expression
    /// text, number of values).
    fn lowered(module: &Module) -> Vec<(String, u32, Option<Fragment>, String, usize)> {
        let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
        let function = function_f(module);
        let q = Query::new(module, function, &layout);
        let markers = AssignmentMarkers::new(function);
        let with_slot = vars_with_stack_slot(function, &markers);
        let mut builder = FunctionVarLocsBuilder::default();
        let prepared =
            build_overlap_map(module, &q, &mut builder, &markers, &with_slot, &llrm_mir::memory::callees(module));
        let size = prepared.tracked_variables;
        let mut lowering = Lowering::new(module, &q, &markers, &with_slot, prepared, builder);
        let mut live = BlockInfo::new(size);
        lowering.process(function.entry().unwrap(), &mut live);
        lowering
            .insert_before
            .iter()
            .flat_map(|(&(inst, slot), locs)| {
                let name = function.instruction(inst).opcode.mnemonic().to_owned();
                let lowering = &lowering;
                locs.iter().map(move |one| {
                    (
                        name.clone(),
                        slot,
                        lowering.builder.variable(one.variable).fragment,
                        expression_text(&one.expression),
                        one.values.len(),
                    )
                })
            })
            .collect()
    }

    /// The records clang wrote for `int a[3]`, followed through the block as
    /// the pin does: the alloca's id makes the whole variable's stack home
    /// match its first `dbg_assign` (a memory location, though its value is
    /// poison); each half's store then matches its `dbg_assign` (memory); and a
    /// store that comes before its record leaves the half without a value
    /// (none) until the record. Derived by hand from the pin's functions.
    #[test]
    fn a_store_and_its_assign_make_a_memory_location() {
        let module = array(TAGGED_1, TAGGED_2);
        let f = |offset, size| Some(Fragment { offset, size });
        assert_eq!(
            lowered(&module),
            [
                ("store".to_owned(), 1, None, "DW_OP_deref".to_owned(), 1),
                ("getelementptr".to_owned(), 0, f(0, 32), "DW_OP_LLVM_fragment, 0, 32".to_owned(), 0),
                ("getelementptr".to_owned(), 1, f(0, 32), "DW_OP_deref, DW_OP_LLVM_fragment, 0, 32".to_owned(), 1),
                ("ret".to_owned(), 0, f(32, 32), "DW_OP_LLVM_fragment, 32, 32".to_owned(), 0),
                (
                    "ret".to_owned(),
                    1,
                    f(32, 32),
                    "DW_OP_plus_uconst, 4, DW_OP_deref, DW_OP_LLVM_fragment, 32, 32".to_owned(),
                    1
                ),
            ]
        );
    }

    /// A call that may write through a pointer to the variable's alloca leaves
    /// whatever it wrote as the variable: memory, whole, from the call on. The
    /// same call with the parameter `readonly` leaves it as it was; before this
    /// the analysis took the call for no write and kept an old value.
    #[test]
    fn a_call_through_the_allocas_pointer_makes_it_a_memory_location() {
        let whole = ("ret".to_owned(), 0, None, "DW_OP_deref".to_owned(), 1);
        let writes = lowered(&array_calling(TAGGED_1, TAGGED_2, "call void @g(ptr %a)"));
        assert!(writes.contains(&whole), "{writes:?}");
        let reads = lowered(&array_calling(TAGGED_1, TAGGED_2, "call void @h(ptr %a)"));
        assert!(!reads.contains(&whole), "{reads:?}");
    }

    /// A store without an id is taken to write its variable's stack home: the
    /// half it lies in is in memory from the store on, at its offset, with
    /// no record to say so.
    #[test]
    fn an_untagged_store_makes_a_memory_location_for_its_variable() {
        let module = array(TAGGED_1, "");
        let locations = lowered(&module);
        assert!(
            locations.contains(&(
                "ret".to_owned(),
                0,
                Some(Fragment { offset: 32, size: 32 }),
                "DW_OP_plus_uconst, 4, DW_OP_deref, DW_OP_LLVM_fragment, 32, 32".to_owned(),
                1
            )),
            "{locations:?}"
        );
    }

    /// `int x` whose only store is tagged and followed by its `dbg_assign`,
    /// in a loop that re-stores it: `x` is in its stack home everywhere, so
    /// the table has one location for it (the bare memory home) and no
    /// wedge, and the back edge does not keep the dataflow from ending.
    const LOOP: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
define i32 @f(i32 %n) {\nentry:\n  %x = alloca i32, align 4, !DIAssignID !24\n\
    #dbg_assign(i1 poison, !20, !DIExpression(), !24, ptr %x, !DIExpression(), !25)\n  br label %loop\n\
loop:\n  %i = phi i32 [ 0, %entry ], [ %j, %loop ]\n  store i32 %i, ptr %x, align 4, !DIAssignID !28\n\
    #dbg_assign(i32 %i, !20, !DIExpression(), !28, ptr %x, !DIExpression(), !25)\n\
  %j = add i32 %i, 1\n  %c = icmp slt i32 %j, %n\n  br i1 %c, label %loop, label %done\n\
done:\n  ret i32 %j\n}\n\
!17 = !DIBasicType(name: \"int\", size: 32, encoding: DW_ATE_signed)\n\
!20 = !DILocalVariable(name: \"x\", scope: !14, type: !17)\n\
!24 = distinct !DIAssignID()\n!28 = distinct !DIAssignID()\n\
!14 = distinct !DISubprogram(name: \"f\")\n!25 = !DILocation(line: 0, scope: !14)\n";

    fn run(text: &str) -> FunctionVarLocs {
        let module = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}"));
        let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
        let function = function_f(&module);
        analyze(&module, &Query::new(&module, function, &layout), true).expect("few blocks")
    }

    #[test]
    fn a_variable_always_in_its_stack_home_has_one_location_and_no_wedge() {
        let locs = run(LOOP);
        assert_eq!(locs.single_locs().len(), 1);
        assert_eq!(locs.single_locs()[0].expression, []);
        assert_eq!(locs.instructions().count(), 0, "no wedge: {:?}", locs.instructions().collect::<Vec<_>>());
    }

    /// The same loop with a `dbg_value` of `x` in the body: `x` is not
    /// always in memory, so it keeps its wedges and has no single location.
    #[test]
    fn a_variable_a_dbg_value_overrides_keeps_its_wedges() {
        let text = LOOP.replace("%j = add", "#dbg_value(i32 %i, !20, !DIExpression(), !25)\n  %j = add");
        assert_ne!(text, LOOP);
        let locs = run(&text);
        assert!(locs.single_locs().is_empty());
        assert!(locs.instructions().count() > 0);
    }

    const WIDE: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
define i64 @f(i64 %w, i32 %n) {\nentry:\n  %x = alloca i64, align 8, !DIAssignID !24\n\
    #dbg_assign(i1 poison, !20, !DIExpression(), !24, ptr %x, !DIExpression(), !25)\n\
  store i64 %w, ptr %x, align 8, !DIAssignID !28\n\
    #dbg_assign(i64 %w, !20, !DIExpression(), !28, ptr %x, !DIExpression(), !25)\n\
    #dbg_value(i32 %n, !20, !DIExpression(DW_OP_LLVM_fragment, 0, 32), !25)\n\
  ret i64 %w\n}\n\
!17 = !DIBasicType(name: \"long\", size: 64, encoding: DW_ATE_signed)\n\
!20 = !DILocalVariable(name: \"x\", scope: !14, type: !17)\n\
!24 = distinct !DIAssignID()\n!28 = distinct !DIAssignID()\n\
!14 = distinct !DISubprogram(name: \"f\")\n!25 = !DILocation(line: 0, scope: !14)\n";

    /// A 64-bit variable in memory whose low half then gets a value: the
    /// debugger takes the high half to have no location from there on, so the
    /// pass states it again as memory (`x[32, 64)` at `alloca + 4`). Without
    /// it the table ended the high half's location at the value.
    #[test]
    fn the_bits_a_value_does_not_cover_are_stated_in_memory_again() {
        let locs = run(WIDE);
        let all: Vec<&VarLocInfo> = locs.instructions().flat_map(|inst| locs.locs_before(inst)).collect();
        let high = all.iter().find(|one| {
            expression_text(&one.expression) == "DW_OP_plus_uconst, 4, DW_OP_deref, DW_OP_LLVM_fragment, 32, 32"
        });
        assert!(high.is_some(), "{:?}", all.iter().map(|one| expression_text(&one.expression)).collect::<Vec<_>>());
        assert_eq!(high.unwrap().values.len(), 1, "at the alloca");
        assert!(locs.variable(high.unwrap().variable).fragment == Some(Fragment { offset: 32, size: 32 }));
    }

    /// Runs the redundancy passes over the entry block of `array()` with the
    /// locations `put` adds, at its first, second and third instruction.
    fn pruned(put: impl FnOnce(&mut FunctionVarLocsBuilder, [InstId; 3], Operand, DebugVariable)) -> Vec<Vec<usize>> {
        let module = array(TAGGED_1, TAGGED_2);
        let function = function_f(&module);
        let insts: Vec<InstId> = function.walk().map(|(_, inst)| inst).collect();
        let value = Operand::Value(function.instruction(insts[0]).result.unwrap());
        let mut builder = FunctionVarLocsBuilder::default();
        let variable = whole(function.debug_records()[0].variable.0);
        put(&mut builder, [insts[0], insts[1], insts[2]], value, variable);
        redundant::run(&module, function, &mut builder, &|_| 0);
        let locs = builder.finish();
        insts[..3].iter().map(|&inst| locs.locs_before(inst).iter().map(|one| one.values.len()).collect()).collect()
    }

    fn whole(node: u32) -> DebugVariable {
        DebugVariable { variable: MetadataId(node), fragment: None, inlined_at: None }
    }

    /// Two locations of the same bytes in one run: only the later one stays.
    /// Both stayed, and a debugger saw the variable have two values at once.
    #[test]
    fn a_location_a_later_one_covers_is_dropped() {
        let kept = pruned(|builder, at, value, variable| {
            builder.add_var_loc((at[0], 0), variable, vec![], None, vec![value]);
            builder.add_var_loc((at[0], 0), variable, vec![DwOp::Deref], None, vec![value, value]);
        });
        assert_eq!(kept[0], [2], "only the later");
    }

    /// A location that says what the variable already is, is dropped.
    #[test]
    fn a_location_that_repeats_the_last_is_dropped() {
        let kept = pruned(|builder, at, value, variable| {
            builder.add_var_loc((at[0], 0), variable, vec![], None, vec![value]);
            builder.add_var_loc((at[1], 0), variable, vec![], None, vec![value]);
        });
        assert_eq!((kept[0].len(), kept[1].len()), (1, 0));
    }

    /// A location of nothing before any real one in the entry block is
    /// dropped; after a real one it stays (the pin's `HasDefinedBits`).
    #[test]
    fn nothing_before_a_real_location_in_the_entry_block_is_dropped() {
        let kept = pruned(|builder, at, value, variable| {
            builder.add_var_loc((at[0], 0), variable, vec![], None, vec![]);
            builder.add_var_loc((at[1], 0), variable, vec![], None, vec![value]);
            builder.add_var_loc((at[2], 0), variable, vec![], None, vec![]);
        });
        assert_eq!((kept[0].len(), kept[1].len(), kept[2].len()), (0, 1, 1));
    }

    /// An expression with `DW_OP_plus` printed as `DW_OP_?`, which no reader
    /// takes back: the table's text could not be compared with the pin's.
    #[test]
    fn an_expression_prints_every_operation_it_keeps() {
        use llrm_mir::module::{DwBinary, DwUnary};
        let ops = [DwOp::Constu(4), DwOp::Binary(DwBinary::Plus), DwOp::Unary(DwUnary::Neg), DwOp::Deref];
        assert_eq!(expression_text(&ops), "DW_OP_constu, 4, DW_OP_plus, DW_OP_neg, DW_OP_deref");
    }

    fn rewritten(text: &str) -> Module {
        let mut module = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}"));
        let locs = run(text);
        let f = module.named("f").unwrap();
        assert!(rewrite(&mut module, f, &locs));
        module
    }

    /// The loop's `#dbg_assign`s stood for a variable always in its stack
    /// home: afterwards one `Declare` of the alloca stands, before the first
    /// instruction, and no `#dbg_assign` does.
    #[test]
    fn a_single_location_becomes_one_declare_ahead_of_the_first_instruction() {
        let module = rewritten(LOOP);
        let function = function_f(&module);
        let first = function.walk().next().unwrap().1;
        let alloca = Operand::Value(function.instruction(first).result.unwrap());
        let records = function.debug_records();
        assert_eq!(records.len(), 1, "{records:?}");
        assert_eq!((records[0].before, records[0].what), (first, DebugWhat::Declare(alloca)));
        assert_eq!(records[0].expression, None);
    }

    /// A variable a `dbg_value` overrides has wedges: each is a `Value`, and
    /// the module prints and reads back with the same records.
    #[test]
    fn wedges_become_values_and_the_module_reads_back() {
        let text = LOOP.replace("%j = add", "#dbg_value(i32 %i, !20, !DIExpression(), !25)\n  %j = add");
        let module = rewritten(&text);
        let records = function_f(&module).debug_records();
        assert!(records.iter().any(|one| matches!(one.what, DebugWhat::Value(_))), "{records:?}");
        assert!(records.iter().all(|one| !matches!(one.what, DebugWhat::Assign { .. })), "{records:?}");
        let printed = llrm_mir::print::module(&module);
        let again = llrm_mir::parse::module(&printed).unwrap_or_else(|error| panic!("{error}\n{printed}"));
        assert_eq!(function_f(&again).debug_records().len(), records.len(), "{printed}");
    }

    /// The pin's `llc -print-debug-ata` on this loop (with the `dbg_value`
    /// of `x` before `%j`): `DW_OP_deref` of the alloca before the entry
    /// branch, then `%i` before the add, and nothing else.
    #[test]
    fn the_loop_with_a_dbg_value_has_the_pins_two_locations() {
        let text = LOOP.replace("%j = add", "#dbg_value(i32 %i, !20, !DIExpression(), !25)\n  %j = add");
        let module = llrm_mir::parse::module(&text).unwrap();
        let function = function_f(&module);
        let locs = run(&text);
        let at =
            |name: &str| function.walk().find(|&(_, i)| function.instruction(i).opcode.mnemonic() == name).unwrap().1;
        let seen: Vec<_> = locs.instructions().map(|inst| (inst, locs.locs_before(inst).len())).collect();
        assert_eq!(seen, [(at("br"), 1), (at("add"), 1)]);
        assert_eq!(locs.locs_before(at("br"))[0].expression, [DwOp::Deref]);
        assert_eq!(locs.locs_before(at("add"))[0].expression, []);
    }

    /// A module read with `#dbg_assign`s leaves the pass with none: the
    /// selector skips them, so each one left a variable without a place.
    #[test]
    fn a_module_leaves_the_pass_without_dbg_assigns() {
        let mut module = llrm_mir::parse::module(LOOP).unwrap();
        module_rewritten(&mut module).unwrap();
        let records = function_f(&module).debug_records();
        assert!(!records.is_empty());
        assert!(records.iter().all(|one| !matches!(one.what, DebugWhat::Assign { .. })), "{records:?}");
    }
}
