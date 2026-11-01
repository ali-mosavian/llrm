//! `AssignmentTrackingAnalysis`: where each variable of a function lives, from
//! its `#dbg_assign` records and the stores they name, as a table of locations
//! the code generator emits.
//!
//! # How it works
//! This file holds the result, [`FunctionVarLocs`], and the table it is built
//! in, [`FunctionVarLocsBuilder`]: the variables (a variable and the bit range
//! of it a location covers), the variables that have one location for their
//! whole scope, and the locations that start just before an instruction. The
//! analysis that fills it comes in later commits; the first part of it is here,
//! the overlap map and the stores it reads.
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

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write;

use llrm_mir::types::Type;
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::nonzero::{get_pointer_offset_from, strip_and_accumulate_constant_offsets};
use llrm_mir::{
    ConstantKind, DebugRecord, DebugWhat, DwOp, Function, InstId, MetadataId, Module, Opcode, Operand, ValueDef,
    ValueId,
};

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
    before: BTreeMap<InstId, Vec<VarLocInfo>>,
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
        inst: InstId,
    ) -> Option<&[VarLocInfo]> {
        self.before.get(&inst).map(Vec::as_slice)
    }

    /// Replaces the locations that start just before `inst` (`setWedge`).
    pub fn set_wedge(
        &mut self,
        inst: InstId,
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
        before: InstId,
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
        FunctionVarLocs { variables: self.variables, single: self.single, before: self.before }
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

/// `buildOverlapMapAndRecordDeclares` without escaping calls and unknown
/// stores: the variables the dataflow tracks, which contain which, and which
/// untagged stores write them; each `#dbg_declare` becomes a location valid
/// for the variable's whole scope.
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

/// The operations of an expression as LLVM prints them.
pub fn expression_text(ops: &[DwOp]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for op in ops {
        parts.push(match op {
            DwOp::PlusUconst(n) => format!("DW_OP_plus_uconst, {n}"),
            DwOp::StackValue => "DW_OP_stack_value".to_owned(),
            DwOp::Fragment { offset, size } => format!("DW_OP_LLVM_fragment, {offset}, {size}"),
            DwOp::Arg(n) => format!("DW_OP_LLVM_arg, {n}"),
            DwOp::Constu(n) => format!("DW_OP_constu, {n}"),
            DwOp::Consts(n) => format!("DW_OP_consts, {n}"),
            DwOp::Deref => "DW_OP_deref".to_owned(),
            DwOp::Binary(_) | DwOp::Unary(_) | DwOp::Other => "DW_OP_?".to_owned(),
        });
    }
    parts.join(", ")
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
        builder.add_var_loc(InstId(3), variable(5), vec![], None, vec![]);
        builder.add_var_loc(InstId(3), variable(6), vec![DwOp::Deref], None, vec![]);
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
  ret i32 %n\n}}\n\
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
        let prepared = build_overlap_map(module, &q, &mut builder, &markers, &with_slot);
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
}
