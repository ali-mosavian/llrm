//! BasicAA, a translation of `BasicAliasAnalysis.cpp` at d1106deb71cc behind
//! the interface of [`crate::aa`] (stage S1/S2 of
//! docs/architecture/basicaa-port.md). Per query: nothing is solved for the
//! whole function, and what one query learns lives in its `AAQueryInfo`.
//!
//! # How it works
//! Translated: `aliasCheck` (empty locations, casts stripped, the same value,
//! the underlying objects, identified objects, arguments against function-local
//! objects, an escape source against a non-captured local object, an access
//! larger than the whole object), `DecomposeGEPExpression` (constant offsets,
//! variable indices by value and scale), `subtractDecomposedGEPs` and
//! `aliasGEP` as far as it needs no value ranges: the inbounds and
//! constant-difference rules, nuw, and the GCD test. Not yet (deviations in the
//! plan page): `GetLinearExpression` through casts, `analyzeVariableOffsets`'
//! ranges, `aliasPHI`, `aliasSelect`, and the mod/ref queries
//! other than `getModRefInfoMask` and `getModRefInfo(Call, Loc)` (they answer
//! the top of the lattice). `getMemoryEffects(Call)` and `getArgModRefInfo`
//! read the attributes.
//!
//! # Example
//! Two allocas never alias, and a call cannot touch an alloca nobody holds a
//! pointer to:
//!
//! ```text
//! %a = alloca i32
//! %b = alloca i32        ; alias(%a, %b) = NoAlias
//! call void @g()         ; getModRefInfo(call, %a) = NoModRef
//! ```
//!
//! # Provides
//! [`BasicAA`], an [`crate::aa::AAResults`]. No client reads it yet.

use llrm_mir::context::{ConstantExpr, ConstantKind};
use llrm_mir::module::{GlobalKind, Instruction, Operand, ValueDef};
use llrm_mir::opcode::{CastOp, Flags, Opcode};

use crate::aa::{AAQueryInfo, AAResults, AliasResult, LocationSize, MemoryEffects, MemoryLocation, ModRefInfo};
use crate::capturetracking::CaptureComponents;
use crate::memory::Unit;

#[cfg(test)]
thread_local! {
    /// Times an object was classified as function-local, for the test that a
    /// batch classifies each object once.
    pub static DECOMPOSITIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub static CLASSIFICATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Fact sets built to read an argument's attributes.
    pub static ARGUMENT_FACTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// `MaxLookupSearchDepth`.
const MAX_LOOKUP: usize = 6;

/// A pointer as a base object, a constant offset and variable indices
/// (`DecomposedGEP`).
#[derive(Clone, Debug)]
pub(crate) struct Decomposed {
    base: Operand,
    offset: i128,
    /// `(index value, scale)`, a scale never zero.
    vars: Vec<(Operand, i128)>,
    inbounds: bool,
    nuw: bool,
}

/// A load or store's place as `DecomposeGEPExpression` finds it: the object its
/// pointer starts from, the constant offset in it (wrapped to the pointer's
/// index width) and the bytes accessed. `indexed` says a variable index is
/// added too, which the offset leaves out.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObjectCell {
    pub object: Operand,
    pub offset: i64,
    pub size: u64,
    pub indexed: bool,
}

/// The place `inst` accesses, if it is a load or store of a known size.
pub fn object_cell(
    unit: &Unit,
    inst: llrm_mir::module::InstId,
) -> Option<ObjectCell> {
    let place = MemoryLocation::get(unit, inst)?;
    let LocationSize::Precise(size) = place.size else { return None };
    let found = BasicAA::new(unit).decomposed(place.ptr);
    let bits = unit.layout.pointer(unit.space(place.ptr).unwrap_or(0)).index_bits;
    let offset = match bits {
        1..=127 => (found.offset << (128 - bits)) >> (128 - bits),
        _ => found.offset,
    };
    Some(ObjectCell { object: found.base, offset: i64::try_from(offset).ok()?, size, indexed: !found.vars.is_empty() })
}

/// Bits of `BasicAA::class`.
struct Class;

impl Class {
    const LOCAL: u8 = 1;
    const GLOBAL_VARIABLE: u8 = 2;
    const ARGUMENT: u8 = 4;
    const ESCAPE_SOURCE: u8 = 8;
    /// `isIdentifiedObject`.
    const IDENTIFIED: u8 = Self::LOCAL | Self::GLOBAL_VARIABLE;
}

/// `isIdentifiedFunctionLocal`: an alloca, a `noalias` call result or a
/// `noalias` or `byval` argument.
pub(crate) fn is_identified_function_local(
    unit: &Unit,
    operand: Operand,
) -> bool {
    BasicAA::new(unit).is_identified_local(operand)
}

/// `CallBase::getMemoryEffects`: the site's attributes met with the direct
/// callee's.
pub(crate) fn call_memory_effects(
    unit: &Unit,
    call: llrm_mir::module::InstId,
) -> MemoryEffects {
    let function = unit.function;
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(call).opcode else {
        return MemoryEffects::unknown();
    };
    let site = MemoryEffects::of_attrs(&info.attrs);
    let callee = llrm_mir::memory::callee(unit.context, function, call)
        .and_then(|one| unit.globals.get(one.0 as usize))
        .and_then(|one| one.function());
    callee.map_or(site, |one| site.intersect(MemoryEffects::of_attrs(&one.attrs)))
}

/// `isIdentifiedObject`: an identified local, or a global variable.
pub(crate) fn is_identified_object(
    unit: &Unit,
    operand: Operand,
) -> bool {
    let basic = BasicAA::new(unit);
    basic.is_identified_local(operand) || basic.global_variable(operand)
}

/// `getUnderlyingObject`: through geps and casts.
pub(crate) fn underlying_object(
    unit: &Unit,
    operand: Operand,
) -> Operand {
    BasicAA::new(unit).underlying(operand)
}

/// `MergeAliasResults`.
fn merge_alias_results(
    one: AliasResult,
    other: AliasResult,
) -> AliasResult {
    use AliasResult::{MayAlias, MustAlias, NoAlias, PartialAlias};
    match (one, other) {
        (NoAlias, NoAlias) => NoAlias,
        (MustAlias, MustAlias) => MustAlias,
        (PartialAlias { .. }, PartialAlias { .. }) => one,
        (PartialAlias { .. }, MustAlias) | (MustAlias, PartialAlias { .. }) => PartialAlias { offset: None },
        _ => MayAlias,
    }
}

pub struct BasicAA<'a> {
    unit: &'a Unit<'a>,
}

impl<'a> BasicAA<'a> {
    pub fn new(unit: &'a Unit<'a>) -> Self {
        Self { unit }
    }

    fn defining(
        &self,
        operand: Operand,
    ) -> Option<&'a Instruction> {
        self.unit.defining(operand).map(|(_, one)| one)
    }

    fn signed(
        &self,
        operand: Operand,
        bits: u128,
    ) -> i128 {
        let width = self.unit.int_bits(operand).unwrap_or(128);
        if width >= 128 || bits >> (width - 1) & 1 == 0 { bits as i128 } else { (bits | (!0u128 << width)) as i128 }
    }

    /// Whether the cast `one` is one a pointer passes through unchanged in
    /// what it points into. A selector alone is `segment:0` when cast back to
    /// far (DEVIATE C1): where an object is below it (`VARSEG(a)`) it is no
    /// pointer into that object; where only an integer is (a segment word
    /// loaded from an array's descriptor) it is the pin's pointer from
    /// `inttoptr`, unless the module forges selectors (`forging`), when it may
    /// name any object.
    fn transparent(
        &self,
        one: &llrm_mir::module::Instruction,
    ) -> bool {
        let selector = one.opcode == Opcode::Cast(CastOp::AddrSpaceCast)
            && self
                .unit
                .space(one.operands[0])
                .is_some_and(|space| self.unit.layout.has_unstable_representation(space));
        if !selector {
            return true;
        }
        let mut below = one.operands[0];
        for _ in 0..MAX_LOOKUP {
            match self.defining(below) {
                Some(next) if matches!(next.opcode, Opcode::Cast(_) | Opcode::GetElementPtr { .. }) => {
                    below = next.operands[0]
                }
                Some(next) if matches!(next.opcode, Opcode::Alloca { .. }) => return false,
                Some(_) => break,
                None => return !matches!(below, Operand::Constant(_)) && !self.forged(),
            }
        }
        !self.forged()
    }

    /// [`Self::transparent`] for a pass that strips casts as the pin's
    /// `stripPointerCasts` does: the selector of an object cast back to a far
    /// pointer is not the pointer it came from.
    pub fn passes_through(
        &self,
        one: &llrm_mir::module::Instruction,
    ) -> bool {
        self.transparent(one)
    }

    /// Whether the module forges pointers into the data segment; without the
    /// module's facts it may.
    fn forged(&self) -> bool {
        self.unit.globals_aa.is_none_or(|globals| globals.forged())
    }

    /// `stripPointerCastsForAliasAnalysis`: casts only.
    fn strip(
        &self,
        mut operand: Operand,
    ) -> Operand {
        loop {
            if let Some(source) = self.constant_cast_source(operand) {
                operand = source;
                continue;
            }
            let Some(one) = self.defining(operand) else { break };
            match one.opcode {
                Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) if self.transparent(one) => {
                    operand = one.operands[0]
                }
                _ => break,
            }
        }
        operand
    }

    /// `isa<GEPOperator>`: a `getelementptr` instruction or constant
    /// expression.
    fn is_gep_operator(
        &self,
        operand: Operand,
    ) -> bool {
        self.constant_gep_base(operand).is_some()
            || self.defining(operand).is_some_and(|one| matches!(one.opcode, Opcode::GetElementPtr { .. }))
    }

    /// The base of a `getelementptr` constant expression, if `operand` is one.
    fn constant_gep_base(
        &self,
        operand: Operand,
    ) -> Option<Operand> {
        match operand {
            Operand::Constant(id) => match &self.unit.context.get(id).kind {
                ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => {
                    Some(Operand::Constant(operands[0]))
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// The operand a `bitcast` or `addrspacecast` constant expression casts,
    /// where a pointer passes through it unchanged (as for the instruction,
    /// [`Self::transparent`]): a selector's own space is not.
    fn constant_cast_source(
        &self,
        operand: Operand,
    ) -> Option<Operand> {
        let Operand::Constant(id) = operand else { return None };
        let ConstantKind::Expr(ConstantExpr::Cast { op: op @ (CastOp::BitCast | CastOp::AddrSpaceCast), value }) =
            &self.unit.context.get(id).kind
        else {
            return None;
        };
        let source = Operand::Constant(*value);
        let selector = *op == CastOp::AddrSpaceCast
            && self.unit.space(source).is_some_and(|space| self.unit.layout.has_unstable_representation(space));
        (!selector).then_some(source)
    }

    /// `getUnderlyingObject`: through geps and casts, `MAX_LOOKUP` steps.
    fn underlying(
        &self,
        mut operand: Operand,
    ) -> Operand {
        for _ in 0..MAX_LOOKUP {
            if let Some(base) = self.constant_gep_base(operand).or_else(|| self.constant_cast_source(operand)) {
                operand = base;
                continue;
            }
            let Some(one) = self.defining(operand) else { break };
            match one.opcode {
                Opcode::GetElementPtr { .. } => operand = one.operands[0],
                Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) if self.transparent(one) => {
                    operand = one.operands[0]
                }
                _ => break,
            }
        }
        operand
    }

    /// `DecomposeGEPExpression`.
    fn decompose(
        &self,
        operand: Operand,
        aaqi: &mut AAQueryInfo,
    ) -> std::rc::Rc<Decomposed> {
        if let Some(found) = aaqi.decomposed.get(&operand) {
            return std::rc::Rc::clone(found);
        }
        let found = std::rc::Rc::new(self.decomposed(operand));
        aaqi.decomposed.insert(operand, std::rc::Rc::clone(&found));
        found
    }

    fn decomposed(
        &self,
        mut operand: Operand,
    ) -> Decomposed {
        #[cfg(test)]
        DECOMPOSITIONS.with(|count| count.set(count.get() + 1));
        let mut found = Decomposed { base: operand, offset: 0, vars: Vec::new(), inbounds: true, nuw: true };
        for _ in 0..MAX_LOOKUP {
            // A `getelementptr` constant expression is a GEPOperator as well:
            // DecomposeGEPExpression reads it as it reads the instruction.
            if let Operand::Constant(id) = operand {
                if let ConstantKind::Expr(ConstantExpr::GetElementPtr { source, inbounds, nuw, operands, .. }) =
                    &self.unit.context.get(id).kind
                {
                    found.inbounds &= *inbounds;
                    found.nuw &= *nuw;
                    let indices: Vec<Option<i128>> = operands[1..]
                        .iter()
                        .map(|&index| {
                            let index = Operand::Constant(index);
                            self.unit.int_constant(index).map(|bits| self.signed(index, bits))
                        })
                        .collect();
                    let (constant, variable) =
                        self.unit.layout.collect_offset(&self.unit.context.types, *source, &indices);
                    debug_assert!(variable.is_empty(), "a constant expression's indices are constants");
                    found.offset += constant;
                    operand = Operand::Constant(operands[0]);
                    found.base = operand;
                    continue;
                }
            }
            if let Some(source) = self.constant_cast_source(operand) {
                operand = source;
                found.base = operand;
                continue;
            }
            let Some(one) = self.defining(operand) else { break };
            match &one.opcode {
                Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) if self.transparent(one) => {
                    operand = one.operands[0]
                }
                // A phi of one arm, as LCSSA makes.
                Opcode::Phi if one.operands.len() == 2 => operand = one.operands[0],
                Opcode::GetElementPtr { source } => {
                    found.inbounds &= one.flags.contains(Flags::INBOUNDS);
                    found.nuw &= one.flags.contains(Flags::NUW);
                    let indices: Vec<Option<i128>> = one.operands[1..]
                        .iter()
                        .map(|&index| self.unit.int_constant(index).map(|bits| self.signed(index, bits)))
                        .collect();
                    let (constant, variable) =
                        self.unit.layout.collect_offset(&self.unit.context.types, *source, &indices);
                    found.offset += constant;
                    for (at, scale) in variable {
                        let (index, scale) = (one.operands[1 + at], scale as i128);
                        match found.vars.iter().position(|(had, _)| *had == index) {
                            Some(have) => {
                                found.vars[have].1 += scale;
                                if found.vars[have].1 == 0 {
                                    found.vars.remove(have);
                                }
                            }
                            None if scale != 0 => found.vars.push((index, scale)),
                            None => {}
                        }
                    }
                    operand = one.operands[0];
                }
                _ => break,
            }
            found.base = operand;
        }
        found.base = operand;
        found
    }

    /// `getUnderlyingObjectAggressive` (ValueTracking.cpp:7380,
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/ValueTracking.cpp#L7380>):
    /// the one object every arm of the selects and phis reaches, else the
    /// plain underlying object.
    pub(crate) fn underlying_aggressive(
        &self,
        operand: Operand,
    ) -> Operand {
        const MAX_VISITED: usize = 8;
        let first = self.underlying(operand);
        let mut work = vec![operand];
        let mut visited = Vec::new();
        let mut object = None;
        let mut is_first = true;
        loop {
            let popped = work.pop().expect("a pending pointer");
            let pointer = if is_first { first } else { self.underlying(popped) };
            is_first = false;
            if !visited.contains(&pointer) {
                visited.push(pointer);
                if visited.len() == MAX_VISITED {
                    return first;
                }
                match self.defining(pointer) {
                    Some(one) if matches!(one.opcode, Opcode::Select) => work.extend_from_slice(&one.operands[1..]),
                    // A phi's operands are value, block pairs.
                    Some(one) if matches!(one.opcode, Opcode::Phi) => work.extend(one.operands.iter().step_by(2)),
                    _ => match object {
                        None => object = Some(pointer),
                        Some(found) if found != pointer => return first,
                        Some(_) => {}
                    },
                }
            }
            if work.is_empty() {
                return object.unwrap_or(first);
            }
        }
    }

    /// `isIdentifiedObject`: any global, not only a variable.
    pub(crate) fn is_identified_object(
        &self,
        operand: Operand,
    ) -> bool {
        self.identified(operand)
            || matches!(
                operand,
                Operand::Constant(id) if matches!(self.unit.context.get(id).kind, ConstantKind::Global(_))
            )
    }

    pub(crate) fn is_alloca(
        &self,
        operand: Operand,
    ) -> bool {
        self.defining(operand).is_some_and(|one| matches!(one.opcode, Opcode::Alloca { .. }))
    }

    pub(crate) fn is_argument(
        &self,
        operand: Operand,
    ) -> bool {
        self.argument(operand)
    }

    fn global_variable(
        &self,
        operand: Operand,
    ) -> bool {
        let Operand::Constant(id) = operand else { return false };
        let ConstantKind::Global(global) = self.unit.context.get(id).kind else { return false };
        matches!(
            self.unit.globals.get(global.0 as usize).map(|one| &one.kind),
            Some(GlobalKind::Variable(_))
        )
    }

    fn argument_attrs(
        &self,
        operand: Operand,
    ) -> Option<&'a [llrm_mir::opcode::Attribute]> {
        let Operand::Value(value) = operand else { return None };
        let ValueDef::Argument(at) = self.unit.function.value(value).def else { return None };
        self.unit.function.parameter_attrs.get(at as usize).map(Vec::as_slice)
    }

    fn noalias_or_byval_argument(
        &self,
        operand: Operand,
    ) -> bool {
        self.argument_attrs(operand).is_some_and(|attrs| {
            llrm_mir::facts::Fact::NoAlias.stated_by(attrs)
                || attrs.iter().any(|one| match one {
                    llrm_mir::opcode::Attribute::Flag(name) | llrm_mir::opcode::Attribute::Type(name, _) => {
                        name == "byval"
                    }
                    _ => false,
                })
        })
    }

    fn noalias_call(
        &self,
        operand: Operand,
    ) -> bool {
        let Operand::Value(value) = operand else { return false };
        let llrm_mir::module::ValueDef::Instruction(inst) = self.unit.function.value(value).def else { return false };
        llrm_mir::memory::returns_noalias(self.unit.context, self.unit.globals, self.unit.function, inst)
    }

    fn in_fixed_space(
        &self,
        pointer: Operand,
    ) -> bool {
        self.unit.space(pointer).is_some_and(|space| self.unit.spaces().is_fixed(space))
    }

    /// Whether `size` bytes at the constant address `pointer` lie wholly in
    /// memory the target states no program data occupies (video, ROM).
    fn in_foreign_memory(
        &self,
        pointer: Operand,
        size: LocationSize,
    ) -> bool {
        let (Some(program), Some(width)) = (self.unit.program, size.value()) else { return false };
        crate::memory::flat_constant(self.unit, pointer)
            .is_some_and(|at| program.target.foreign_span((0, 0), (at, at), width as i64).is_some())
    }

    /// A global variable or an alloca outside the fixed-address space.
    fn program_object(
        &self,
        object: Operand,
    ) -> bool {
        (self.global_variable(object) || self.is_alloca(object)) && !self.in_fixed_space(object)
    }

    /// The result of `llrm.noalias`: reached by no pointer not based on it.
    fn restrict_root(
        &self,
        object: Operand,
    ) -> bool {
        self.unit
            .defining(object)
            .is_some_and(
                |(inst, one)| matches!(one.opcode, Opcode::Call(_))
                    && self.unit.intrinsic(inst) == Some(llrm_mir::intrinsics::Intrinsic::Restrict),
            )
    }

    /// `isIdentifiedFunctionLocal`, counted for the test that a batch
    /// classifies each object once.
    fn identified_local(
        &self,
        operand: Operand,
    ) -> bool {
        #[cfg(test)]
        CLASSIFICATIONS.with(|count| count.set(count.get() + 1));
        self.is_identified_local(operand)
    }

    fn is_identified_local(
        &self,
        operand: Operand,
    ) -> bool {
        self.defining(operand).is_some_and(|one| matches!(one.opcode, Opcode::Alloca { .. }))
            || self.noalias_call(operand)
            || self.noalias_or_byval_argument(operand)
    }

    /// `isIdentifiedObject`.
    fn identified(
        &self,
        operand: Operand,
    ) -> bool {
        self.identified_local(operand) || self.global_variable(operand)
    }

    /// What `operand` is, as `Class` bits, found once for the batch.
    fn class(
        &self,
        operand: Operand,
        aaqi: &mut AAQueryInfo,
    ) -> u8 {
        if let Some(found) = aaqi.classes.get(&operand) {
            return *found;
        }
        let mut class = 0;
        if self.identified_local(operand) {
            class |= Class::LOCAL;
        }
        if self.global_variable(operand) {
            class |= Class::GLOBAL_VARIABLE;
        }
        if self.argument(operand) {
            class |= Class::ARGUMENT;
        }
        if self.escape_source(operand) {
            class |= Class::ESCAPE_SOURCE;
        }
        aaqi.classes.insert(operand, class);
        class
    }

    fn argument(
        &self,
        operand: Operand,
    ) -> bool {
        matches!(
            operand,
            Operand::Value(value) if matches!(self.unit.function.value(value).def, ValueDef::Argument(_))
        )
    }

    /// `isEscapeSource`: a call that may return what it was given, a load, an
    /// `inttoptr`.
    fn escape_source(
        &self,
        operand: Operand,
    ) -> bool {
        self.defining(operand).is_some_and(|one| {
            matches!(
                one.opcode,
                Opcode::Call(_) | Opcode::Invoke(_) | Opcode::Load { .. } | Opcode::Cast(CastOp::IntToPtr)
            )
        })
    }

    /// `AAQI.CA->getCapturesBefore(Object, I, OrAt, ReturnCaptures)`: asked of
    /// the batch's capture analysis (`SimpleCaptureAnalysis`: the whole
    /// function; `EarliestEscapeAnalysis`: before `inst`).
    fn captures_before(
        &self,
        object: Operand,
        inst: Option<llrm_mir::module::InstId>,
        or_at: bool,
        return_captures: bool,
        aaqi: &mut AAQueryInfo,
    ) -> CaptureComponents {
        aaqi.capture.captures_before(self.unit, object, inst, or_at, return_captures)
    }

    /// Whether the address of `object` may be captured anywhere, a return
    /// included.
    pub(crate) fn is_captured(
        &self,
        object: Operand,
        aaqi: &mut AAQueryInfo,
    ) -> bool {
        !self.captures_before(object, None, true, true, aaqi).captures_nothing()
    }

    /// Whether the address of `object` may be captured before `at` (anywhere
    /// when `at` is none), a return not counted.
    pub(crate) fn is_captured_before(
        &self,
        object: Operand,
        at: Option<llrm_mir::module::InstId>,
        aaqi: &mut AAQueryInfo,
    ) -> bool {
        !self.captures_before(object, at, true, false, aaqi).captures_nothing()
    }

    /// `capturesNothing(getCapturesBefore(O, dyn_cast<Instruction>(Source),
    /// /*OrAt=*/true, /*ReturnCaptures=*/false))`.
    fn not_captured_before(
        &self,
        object: Operand,
        source: Operand,
        aaqi: &mut AAQueryInfo,
    ) -> bool {
        let at = self.unit.defining(source).map(|(inst, _)| inst);
        self.captures_before(object, at, true, false, aaqi).captures_nothing()
    }

    /// `isObjectSmallerThan(object, size)`: an identified object whose whole
    /// extent is less than the access.
    fn object_smaller_than(
        &self,
        object: Operand,
        size: u64,
        aaqi: &mut AAQueryInfo,
    ) -> bool {
        self.class(object, aaqi) & Class::IDENTIFIED != 0
            && crate::memory::extent_of(self.unit, object).is_some_and(|extent| (extent as u64) < size)
    }

    fn is_object_size(
        &self,
        object: Operand,
        size: u64,
        aaqi: &mut AAQueryInfo,
    ) -> bool {
        self.class(object, aaqi) & Class::IDENTIFIED != 0
            && crate::memory::extent_of(self.unit, object) == Some(size as i64)
    }

    fn alias_check(
        &self,
        a: &MemoryLocation,
        b: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
    ) -> AliasResult {
        if a.size.is_zero() || b.size.is_zero() {
            return AliasResult::NoAlias;
        }
        let (v1, v2) = (self.strip(a.ptr), self.strip(b.ptr));
        // Looking through phis, values of two iterations may be compared: the
        // same value is the same only outside a cycle
        // (`isValueEqualInPotentialCycles`).
        if self.equal_in_potential_cycles(v1, v2, aaqi) {
            return AliasResult::MustAlias;
        }
        let (o1, o2) = (self.underlying(v1), self.underlying(v2));
        if o1 != o2 {
            // A fixed-address pointer is no global or frame object (not LLVM's:
            // it has no such space, #2555).
            // The same of a constant address the target states is foreign.
            if ((self.in_fixed_space(v1) || self.in_foreign_memory(v1, a.size)) && self.program_object(o2))
                || ((self.in_fixed_space(v2) || self.in_foreign_memory(v2, b.size)) && self.program_object(o1))
            {
                return AliasResult::NoAlias;
            }
            // Pointers the pass made restrict, from different roots (not
            // LLVM's).
            if self.restrict_root(o1) && self.restrict_root(o2) {
                return AliasResult::NoAlias;
            }
            // Two different identified objects.
            let (c1, c2) = (self.class(o1, aaqi), self.class(o2, aaqi));
            if c1 & Class::IDENTIFIED != 0 && c2 & Class::IDENTIFIED != 0 {
                return AliasResult::NoAlias;
            }
            // Arguments cannot alias what is identified at the function level.
            if (c1 & Class::ARGUMENT != 0 && c2 & Class::LOCAL != 0)
                || (c2 & Class::ARGUMENT != 0 && c1 & Class::LOCAL != 0)
            {
                return AliasResult::NoAlias;
            }
            // A call or load result cannot be a non-escaping local object.
            if (c1 & Class::ESCAPE_SOURCE != 0 && self.not_captured_before(o2, o1, aaqi))
                || (c2 & Class::ESCAPE_SOURCE != 0 && self.not_captured_before(o1, o2, aaqi))
            {
                return AliasResult::NoAlias;
            }
        }
        // An access larger than the entire object on the other side is
        // undefined.
        if b.size.value().is_some_and(|size| self.object_smaller_than(o1, size, aaqi))
            || a.size.value().is_some_and(|size| self.object_smaller_than(o2, size, aaqi))
        {
            return AliasResult::NoAlias;
        }
        // An access that may be before its pointer is canonicalised to an
        // unbounded one.
        let (s1, s2) = if a.size.may_be_before_pointer() || b.size.may_be_before_pointer() {
            (LocationSize::AfterPointer, LocationSize::AfterPointer)
        } else {
            (a.size, b.size)
        };
        let (l1, l2) = (MemoryLocation { ptr: v1, size: s1, ..*a }, MemoryLocation { ptr: v2, size: s2, ..*b });
        let mark = match aaqi.enter(&l1, &l2) {
            crate::aa::Entered::Known(found) => return found,
            crate::aa::Entered::Fresh(mark) => mark,
        };
        let result = self.alias_recursive(&l1, &l2, o1, o2, aaqi);
        aaqi.exit(mark, result)
    }

    fn alias_recursive(
        &self,
        a: &MemoryLocation,
        b: &MemoryLocation,
        o1: Operand,
        o2: Operand,
        aaqi: &mut AAQueryInfo,
    ) -> AliasResult {
        let is_gep = |operand: Operand| self.is_gep_operator(operand);
        if is_gep(a.ptr) {
            let result = self.alias_gep(a, b, o1, o2, aaqi);
            if result != AliasResult::MayAlias {
                return result;
            }
        } else if is_gep(b.ptr) {
            let result = self.alias_gep(b, a, o2, o1, aaqi).swap();
            if result != AliasResult::MayAlias {
                return result;
            }
        }
        // `aliasPHI` of V1, else of V2; then `aliasSelect` the same way.
        let defines =
            |operand: Operand, wanted: fn(&Opcode) -> bool| self.defining(operand).filter(|one| wanted(&one.opcode));
        let is_phi: fn(&Opcode) -> bool = |opcode| matches!(opcode, Opcode::Phi);
        let is_select: fn(&Opcode) -> bool = |opcode| matches!(opcode, Opcode::Select);
        if let Some(one) = defines(a.ptr, is_phi) {
            let result = self.alias_phi(one, a.ptr, a.size, b.ptr, b.size, aaqi);
            if result != AliasResult::MayAlias {
                return result;
            }
        } else if let Some(one) = defines(b.ptr, is_phi) {
            let result = self.alias_phi(one, b.ptr, b.size, a.ptr, a.size, aaqi).swap();
            if result != AliasResult::MayAlias {
                return result;
            }
        }
        if let Some(one) = defines(a.ptr, is_select) {
            let result = self.alias_select(one, a.size, b.ptr, b.size, aaqi);
            if result != AliasResult::MayAlias {
                return result;
            }
        } else if let Some(one) = defines(b.ptr, is_select) {
            let result = self.alias_select(one, b.size, a.ptr, a.size, aaqi).swap();
            if result != AliasResult::MayAlias {
                return result;
            }
        }
        // Both inside one object, one access covering all of it.
        if o1 == o2
            && a.size.is_precise()
            && b.size.is_precise()
            && (self.is_object_size(o1, a.size.value().unwrap_or(0), aaqi)
                || self.is_object_size(o2, b.size.value().unwrap_or(0), aaqi))
        {
            return AliasResult::PartialAlias { offset: None };
        }
        AliasResult::MayAlias
    }

    /// `isValueEqualInPotentialCycles`: DEVIATE AP2: across iterations an
    /// instruction outside the entry block is never taken to be the same
    /// value in both (the pin asks `isNotInCycle`).
    fn equal_in_potential_cycles(
        &self,
        one: Operand,
        other: Operand,
        aaqi: &AAQueryInfo,
    ) -> bool {
        if one != other {
            return false;
        }
        if !aaqi.may_be_cross_iteration {
            return true;
        }
        match self.unit.defining(one) {
            None => true,
            Some((inst, _)) => self.unit.function.parent(inst) == self.unit.function.entry(),
        }
    }

    /// `aliasSelect`: DEVIATE AP1: the arms are asked of BasicAA alone, where
    /// the pin asks the whole stack (`AAQI.AAR`).
    fn alias_select(
        &self,
        select: &llrm_mir::module::Instruction,
        size: LocationSize,
        other: Operand,
        other_size: LocationSize,
        aaqi: &mut AAQueryInfo,
    ) -> AliasResult {
        let at = |ptr: Operand, size: LocationSize| MemoryLocation::new(ptr, size);
        let (condition, on_true, on_false) = (select.operands[0], select.operands[1], select.operands[2]);
        // Selects with the same condition: the values on corresponding arms.
        if let Some(second) = self.defining(other)
            && matches!(second.opcode, Opcode::Select)
            && self.equal_in_potential_cycles(condition, second.operands[0], aaqi)
        {
            let alias = self.alias_check(&at(on_true, size), &at(second.operands[1], other_size), aaqi);
            if alias == AliasResult::MayAlias {
                return AliasResult::MayAlias;
            }
            let this = self.alias_check(&at(on_false, size), &at(second.operands[2], other_size), aaqi);
            return merge_alias_results(this, alias);
        }
        // Both arms NoAlias or MustAlias the other: that.
        let alias = self.alias_check(&at(on_true, size), &at(other, other_size), aaqi);
        if alias == AliasResult::MayAlias {
            return AliasResult::MayAlias;
        }
        let this = self.alias_check(&at(on_false, size), &at(other, other_size), aaqi);
        merge_alias_results(this, alias)
    }

    /// `aliasPHI`. DEVIATE AP1 as for `alias_select`.
    fn alias_phi(
        &self,
        phi: &llrm_mir::module::Instruction,
        phi_operand: Operand,
        mut size: LocationSize,
        other: Operand,
        other_size: LocationSize,
        aaqi: &mut AAQueryInfo,
    ) -> AliasResult {
        let at = |ptr: Operand, size: LocationSize| MemoryLocation::new(ptr, size);
        let incoming: Vec<(Operand, Operand)> = phi.operands.chunks(2).map(|pair| (pair[0], pair[1])).collect();
        if incoming.is_empty() {
            return AliasResult::NoAlias;
        }
        // Phis of one block: the values on corresponding edges, unless across
        // iterations (another entry may be taken in another iteration).
        if let Some((second_inst, second)) = self.unit.defining(other)
            && matches!(second.opcode, Opcode::Phi)
            && let Some((first_inst, _)) = self.unit.defining(phi_operand)
            && self.unit.function.parent(second_inst) == self.unit.function.parent(first_inst)
            && !aaqi.may_be_cross_iteration
        {
            let mut result: Option<AliasResult> = None;
            for &(value, block) in &incoming {
                let Some(partner) = second.operands.chunks(2).find(|pair| pair[1] == block).map(|pair| pair[0]) else {
                    return AliasResult::MayAlias;
                };
                let this = self.alias_check(&at(value, size), &at(partner, other_size), aaqi);
                let merged = result.map_or(this, |so_far| merge_alias_results(so_far, this));
                result = Some(merged);
                if merged == AliasResult::MayAlias {
                    break;
                }
            }
            return result.unwrap_or(AliasResult::MayAlias);
        }
        // A phi operand that recurses to the phi leaves the other operands'
        // underlying objects to decide.
        let mut sources: Vec<Operand> = Vec::new();
        let mut is_recursive = false;
        let mut one_phi: Option<Operand> = None;
        for &(value, _) in &incoming {
            // The phi itself as an incoming value is skipped.
            if value == phi_operand {
                continue;
            }
            if self.defining(value).is_some_and(|one| matches!(one.opcode, Opcode::Phi)) {
                if one_phi.is_some_and(|seen| seen != value) {
                    // More than one phi input: conservative.
                    return AliasResult::MayAlias;
                }
                one_phi = Some(value);
            }
            if self.underlying(value) == phi_operand {
                is_recursive = true;
                continue;
            }
            if !sources.contains(&value) {
                sources.push(value);
            }
        }
        if one_phi.is_some() && sources.len() > 1 {
            // Only the trivial lcssa and recursive phi cases.
            return AliasResult::MayAlias;
        }
        if sources.is_empty() {
            return AliasResult::MayAlias;
        }
        // A recursive phi moves across iterations.
        if is_recursive {
            size = LocationSize::BeforeOrAfterPointer;
        }
        // The queries below may compare values of two loop iterations.
        let was = std::mem::replace(&mut aaqi.may_be_cross_iteration, true);
        let result = (|| {
            let mut alias = self.alias_check(&at(sources[0], size), &at(other, other_size), aaqi);
            if alias == AliasResult::MayAlias {
                return AliasResult::MayAlias;
            }
            // A recursive phi proves nothing but NoAlias.
            if is_recursive && alias != AliasResult::NoAlias {
                return AliasResult::MayAlias;
            }
            for &source in &sources[1..] {
                let this = self.alias_check(&at(source, size), &at(other, other_size), aaqi);
                alias = merge_alias_results(this, alias);
                if alias == AliasResult::MayAlias {
                    break;
                }
            }
            alias
        })();
        aaqi.may_be_cross_iteration = was;
        result
    }

    /// `aliasGEP`, without the range reasoning over variable indices.
    fn alias_gep(
        &self,
        gep: &MemoryLocation,
        other: &MemoryLocation,
        underlying1: Operand,
        underlying2: Operand,
        aaqi: &mut AAQueryInfo,
    ) -> AliasResult {
        let (mut size1, mut size2) = (gep.size, other.size);
        let base_objects = |this: &Self, aaqi: &mut AAQueryInfo| {
            let result = this.alias_check(
                &MemoryLocation::before_or_after(underlying1),
                &MemoryLocation::before_or_after(underlying2),
                aaqi,
            );
            if result == AliasResult::NoAlias { AliasResult::NoAlias } else { AliasResult::MayAlias }
        };
        if size1.value().is_none() && size2.value().is_none() {
            let is_join =
                self.defining(other.ptr).is_some_and(|one| matches!(one.opcode, Opcode::Phi | Opcode::Select));
            if is_join {
                return AliasResult::MayAlias;
            }
            let joins = |operand| {
                self.is_gep_operator(operand)
                    || self.defining(operand).is_some_and(|one| matches!(one.opcode, Opcode::Phi | Opcode::Select))
            };
            if joins(other.ptr) || joins(underlying1) || joins(underlying2) {
                return base_objects(self, aaqi);
            }
            return AliasResult::MayAlias;
        }
        let (mut d1, mut d2) = ((*self.decompose(gep.ptr, aaqi)).clone(), (*self.decompose(other.ptr, aaqi)).clone());
        if d1.base == gep.ptr && d2.base == other.ptr {
            return AliasResult::MayAlias;
        }
        // Swap if the second has more variable indices.
        if d1.vars.len() < d2.vars.len() {
            std::mem::swap(&mut d1, &mut d2);
            std::mem::swap(&mut size1, &mut size2);
        }
        // d1 - d2.
        d1.nuw &= d1.offset >= d2.offset;
        d1.offset -= d2.offset;
        for (value, scale) in d2.vars.iter().copied() {
            match d1.vars.iter().position(|(had, _)| *had == value) {
                Some(have) => {
                    if d1.vars[have].1 != scale {
                        d1.nuw &= d1.vars[have].1 >= scale;
                        d1.vars[have].1 -= scale;
                    } else {
                        d1.vars.remove(have);
                    }
                }
                None => {
                    d1.vars.push((value, -scale));
                    d1.nuw = false;
                }
            }
        }
        // An inbounds gep that would have to start out of bounds for the two to
        // alias.
        let base_of_object = |operand: Operand| {
            self.global_variable(operand)
                || self.defining(operand).is_some_and(|one| matches!(one.opcode, Opcode::Alloca { .. }))
        };
        if d1.inbounds
            && d1.vars.is_empty()
            && size2.value().is_some()
            && d1.offset >= i128::from(size2.value().unwrap_or(0))
            && base_of_object(d2.base)
        {
            return AliasResult::NoAlias;
        }
        if d2.inbounds
            && d1.vars.is_empty()
            && size1.value().is_some()
            && d1.offset <= -i128::from(size1.value().unwrap_or(0))
            && base_of_object(d1.base)
        {
            return AliasResult::NoAlias;
        }
        // Equal offsets: the sizes are kept for the check of the bases.
        if d1.offset == 0 && d1.vars.is_empty() {
            return self.alias_check(
                &MemoryLocation { ptr: d1.base, size: size1, aa_info: gep.aa_info },
                &MemoryLocation { ptr: d2.base, size: size2, aa_info: other.aa_info },
                aaqi,
            );
        }
        let base_alias = self.alias_check(
            &MemoryLocation::before_or_after(d1.base),
            &MemoryLocation::before_or_after(d2.base),
            aaqi,
        );
        if base_alias != AliasResult::MustAlias {
            return base_alias;
        }
        if d1.vars.is_empty() {
            let (mut offset, mut left, mut right) = (d1.offset, size2, size1);
            let swapped = offset < 0;
            if swapped {
                std::mem::swap(&mut left, &mut right);
                offset = -offset;
            }
            let Some(left_bytes) = left.value() else { return AliasResult::MayAlias };
            if offset < i128::from(left_bytes) {
                let nested = right.value().is_some_and(|bytes| offset + i128::from(bytes) <= i128::from(left_bytes));
                if nested {
                    let shift = i32::try_from(-offset).ok();
                    let result = AliasResult::PartialAlias { offset: shift };
                    return if swapped { result.swap() } else { result };
                }
                return AliasResult::PartialAlias { offset: None };
            }
            return AliasResult::NoAlias;
        }
        // The distance is nuw-bounded below by the constant part.
        if d1.nuw && size2.value().is_some_and(|bytes| d1.offset >= i128::from(bytes)) {
            return AliasResult::NoAlias;
        }
        let (Some(bytes1), Some(bytes2)) = (size1.value(), size2.value()) else { return AliasResult::MayAlias };
        // The variable part is a multiple of the GCD of the scales (a scale
        // without nsw is known only down to its lowest set bit).
        let gcd = d1.vars.iter().map(|(_, scale)| 1i128 << scale.unsigned_abs().trailing_zeros().min(100)).fold(0, gcd);
        if gcd > 0 {
            let modulus = d1.offset.rem_euclid(gcd);
            if modulus >= i128::from(bytes2) && gcd - modulus >= i128::from(bytes1) {
                return AliasResult::NoAlias;
            }
        }
        AliasResult::MayAlias
    }
}

impl BasicAA<'_> {
    /// `getModRefInfo(Call, Loc)`: what the call does to the location, from
    /// the call's memory effects and, for what it reaches only through
    /// arguments, from which arguments may alias the location.
    fn call_mod_ref(
        &mut self,
        call: llrm_mir::module::InstId,
        loc: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
        mut others: Option<
            &mut dyn FnMut(
                &MemoryLocation,
                &MemoryLocation,
                &mut AAQueryInfo,
                Option<llrm_mir::module::InstId>,
            ) -> AliasResult,
        >,
    ) -> ModRefInfo {
        use crate::aa::IRMemLocation::{ArgMem, ErrnoMem, InaccessibleMem, Other};
        let op = self.unit.function.instruction(call);
        let (Opcode::Call(info) | Opcode::Invoke(info)) = &op.opcode else { return ModRefInfo::ModRef };
        let object = self.underlying(loc.ptr);
        let local_slot = self.defining(object).is_some_and(|one| matches!(one.opcode, Opcode::Alloca { .. }));
        // A tail call cannot reach this frame's slots, but may take one by
        // value.
        let by_value = |attrs: &[llrm_mir::opcode::Attribute]| {
            attrs
                .iter()
                .any(
                    |one| matches!(
                        one,
                        llrm_mir::opcode::Attribute::Flag(name) | llrm_mir::opcode::Attribute::Type(name, _) if name == "byval"
                    ),
                )
        };
        if local_slot
            && info.tail == llrm_mir::opcode::Tail::Tail
            && !(by_value(&info.attrs)
                || by_value(&info.return_attrs)
                || info.argument_attrs.iter().any(|one| by_value(one)))
        {
            return ModRefInfo::NoModRef;
        }
        let effects = self.get_memory_effects(call, aaqi).with(InaccessibleMem, ModRefInfo::NoModRef);
        if effects.does_not_access_memory() {
            return ModRefInfo::NoModRef;
        }
        let mut arg = effects.mod_ref(ArgMem);
        let errno = effects.mod_ref(ErrnoMem);
        let mut other = effects.mod_ref(Other);
        // What does not escape is reached through arguments only.
        let callee =
            llrm_mir::memory::callee(self.unit.context, self.unit.function, call).and_then(|one| {
                match &self.unit.globals.get(one.0 as usize)?.kind {
                    GlobalKind::Function(function) => Some(function),
                    GlobalKind::Variable(_) => None,
                }
            });
        let returns_twice = llrm_mir::memory::has(&info.attrs, "returns_twice")
            || callee.is_some_and(|one| llrm_mir::memory::has(&one.attrs, "returns_twice"));
        let is_call_itself = matches!(object, Operand::Value(value) if op.result == Some(value));
        if other != ModRefInfo::NoModRef
            && matches!(object, Operand::Value(_))
            && !is_call_itself
            && (local_slot || !returns_twice)
        {
            let captures = self.captures_before(object, Some(call), false, false, aaqi);
            if captures.captures_nothing() {
                other = ModRefInfo::NoModRef;
            } else if captures == CaptureComponents::READ_PROVENANCE {
                other = other.intersect(ModRefInfo::Ref);
            }
        }
        if arg.union(other) != other {
            let mut narrowed = ModRefInfo::NoModRef;
            let arguments = op.operands.len().saturating_sub(1);
            for at in 0..arguments {
                if self.unit.space(op.operands[at]).is_none() {
                    continue;
                }
                let at = at as u32;
                let Some(place) = MemoryLocation::get_for_argument(self.unit, call, at) else { continue };
                let mut verdict = AliasResult::MayAlias;
                if let Some(others) = others.as_mut() {
                    verdict = others(&place, loc, aaqi, Some(call));
                }
                if verdict == AliasResult::MayAlias {
                    verdict = self.alias_check(&place, loc, aaqi);
                }
                if verdict != AliasResult::NoAlias {
                    narrowed = narrowed.union(arg.intersect(self.get_arg_mod_ref_info(call, at)));
                }
                if narrowed == arg {
                    break;
                }
            }
            arg = narrowed;
        }
        let mut result = arg.union(other);
        // aliasErrno without the target's int size: only a local slot or an
        // access wider than an int (32 bits) is told apart.
        if errno.union(result) != result {
            let apart =
                self.class(object, aaqi) & Class::LOCAL != 0 || loc.size.value().is_some_and(|bytes| bytes * 8 > 32);
            if !apart {
                result = result.union(errno);
            }
        }
        if result != ModRefInfo::ModRef {
            return result;
        }
        let invariant_start = llrm_mir::memory::callee(self.unit.context, self.unit.function, call)
            .and_then(|one| self.unit.globals.get(one.0 as usize)?.name.as_deref())
            .is_some_and(|name| name.starts_with("llvm.invariant.start"));
        if invariant_start { ModRefInfo::Ref } else { ModRefInfo::ModRef }
    }
}

fn gcd(
    a: i128,
    b: i128,
) -> i128 {
    if b == 0 { a } else { gcd(b, a % b) }
}

impl AAResults for BasicAA<'_> {
    fn alias(
        &mut self,
        a: &MemoryLocation,
        b: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
        _ctx: Option<llrm_mir::module::InstId>,
    ) -> AliasResult {
        self.alias_check(a, b, aaqi)
    }

    /// `getModRefInfoMask`: a location in a constant global or a
    /// `noalias readonly` argument is never written; one in no object this
    /// walk (through selects and phis of up to eight arms) names is
    /// anything.
    fn get_mod_ref_info_mask(
        &mut self,
        loc: &MemoryLocation,
        _: &mut AAQueryInfo,
        ignore_locals: bool,
    ) -> ModRefInfo {
        let mut max_lookup = 8;
        let mut worklist = vec![loc.ptr];
        let mut visited: Vec<Operand> = Vec::new();
        let mut result = ModRefInfo::NoModRef;
        // `do { ... } while (!Worklist.empty() && --MaxLookup)`: a `continue`
        // of the C++ still counts a step.
        loop {
            let next = worklist.pop().expect("a non-empty worklist");
            'step: {
                let object = self.underlying(next);
                if visited.contains(&object) {
                    break 'step;
                }
                visited.push(object);
                let defining = self.defining(object);
                if ignore_locals && defining.is_some_and(|one| matches!(one.opcode, Opcode::Alloca { .. })) {
                    break 'step;
                }
                if let Some(attrs) = self.argument_attrs(object) {
                    let facts = llrm_mir::facts::Facts::of(attrs);
                    if facts.no_alias() && facts.read_only() {
                        result = result.union(ModRefInfo::Ref);
                        break 'step;
                    }
                }
                if let Operand::Constant(id) = object {
                    if let ConstantKind::Global(global) = self.unit.context.get(id).kind {
                        match self.unit.globals.get(global.0 as usize).map(|one| &one.kind) {
                            Some(GlobalKind::Variable(variable)) if variable.constant => break 'step,
                            _ => return ModRefInfo::ModRef,
                        }
                    }
                }
                match defining.map(|one| (&one.opcode, &one.operands)) {
                    Some((Opcode::Select, operands)) => worklist.extend([operands[1], operands[2]]),
                    Some((Opcode::Phi, operands)) => {
                        if operands.len() / 2 > 8 {
                            return ModRefInfo::ModRef;
                        }
                        worklist.extend(operands.iter().step_by(2).copied());
                    }
                    _ => return ModRefInfo::ModRef,
                }
            }
            max_lookup -= 1;
            if worklist.is_empty() || max_lookup == 0 {
                break;
            }
        }
        if worklist.is_empty() { result } else { ModRefInfo::ModRef }
    }

    /// `BasicAAResult::getArgModRefInfo` (BasicAliasAnalysis.cpp:897,
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/BasicAliasAnalysis.cpp#L897>):
    /// the call site's attributes on the argument, else the callee's on the
    /// parameter (`CallBase::paramHasAttr`).
    fn get_arg_mod_ref_info(
        &mut self,
        call: llrm_mir::module::InstId,
        arg: u32,
    ) -> ModRefInfo {
        let function = self.unit.function;
        let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(call).opcode else {
            return ModRefInfo::ModRef;
        };
        let callee = llrm_mir::memory::callee(self.unit.context, function, call)
            .and_then(|one| self.unit.globals.get(one.0 as usize))
            .and_then(|one| one.function());
        // The facts of the site's attributes and of the callee's parameter,
        // made once (each `Facts::of` allocates; most arguments state
        // none).
        let facts_of = |attrs: Option<&Vec<llrm_mir::opcode::Attribute>>| {
            attrs
                .filter(|attrs| !attrs.is_empty())
                .map(
                    |attrs| {
                        #[cfg(test)]
                        ARGUMENT_FACTS.with(|made| made.set(made.get() + 1));
                        llrm_mir::facts::Facts::of(attrs)
                    },
                )
        };
        let site = facts_of(info.argument_attrs.get(arg as usize));
        let parameter = facts_of(callee.and_then(|one| one.parameter_attrs.get(arg as usize)));
        let stated = |fact: fn(&llrm_mir::facts::Facts) -> bool| {
            site.as_ref().is_some_and(|one| fact(one)) || parameter.as_ref().is_some_and(|one| fact(one))
        };
        if stated(llrm_mir::facts::Facts::read_none) {
            ModRefInfo::NoModRef
        } else if stated(llrm_mir::facts::Facts::write_only) {
            ModRefInfo::Mod
        } else if stated(llrm_mir::facts::Facts::read_only) {
            ModRefInfo::Ref
        } else {
            ModRefInfo::ModRef
        }
    }

    /// `BasicAAResult::getMemoryEffects(Call)` (BasicAliasAnalysis.cpp:860,
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/BasicAliasAnalysis.cpp#L860>):
    /// the call site's `memory(...)` met with the direct callee's.
    ///
    /// DEVIATE B1: no `hasReadingOperandBundles`/`hasClobberingOperandBundles`/
    /// `isVolatile` terms. MIR has no operand bundles (#2203) and no volatile
    /// call; `CallInfo` carries neither, so there is nothing to read.
    /// DEVIATE B2: no `experimental_guard`/`experimental_deoptimize` case; MIR
    /// has neither intrinsic.
    fn get_memory_effects(
        &mut self,
        call: llrm_mir::module::InstId,
        _: &mut AAQueryInfo,
    ) -> MemoryEffects {
        call_memory_effects(self.unit, call)
    }

    /// A call against a location is `getModRefInfo(Call, Loc)`; every other
    /// instruction is not answered yet.
    fn get_mod_ref_info(
        &mut self,
        inst: llrm_mir::module::InstId,
        loc: Option<&MemoryLocation>,
        aaqi: &mut AAQueryInfo,
    ) -> ModRefInfo {
        // `AAResults::getModRefInfo(Instruction *, Loc)`: by opcode, and
        // nothing for the rest.
        match self.unit.function.instruction(inst).opcode {
            Opcode::Call(_) | Opcode::Invoke(_) => match loc {
                Some(loc) => self.call_mod_ref(inst, loc, aaqi, None),
                None => ModRefInfo::ModRef,
            },
            // A load that cannot alias the location neither reads nor writes
            // it; otherwise it reads. MIR has no atomics to be stronger than
            // unordered.
            Opcode::Load { .. } => {
                if let (Some(loc), Some(own)) = (loc, MemoryLocation::get(self.unit, inst))
                    && self.alias(&own, loc, aaqi, Some(inst)) == AliasResult::NoAlias
                {
                    return ModRefInfo::NoModRef;
                }
                ModRefInfo::Ref
            }
            // A store that cannot alias the location, or writes to constant
            // memory, does not modify it; otherwise it writes.
            Opcode::Store { .. } => {
                if let (Some(loc), Some(own)) = (loc, MemoryLocation::get(self.unit, inst)) {
                    if self.alias(&own, loc, aaqi, Some(inst)) == AliasResult::NoAlias {
                        return ModRefInfo::NoModRef;
                    }
                    if self.points_to_constant_memory(loc, aaqi, false) {
                        return ModRefInfo::NoModRef;
                    }
                }
                ModRefInfo::Mod
            }
            _ => ModRefInfo::NoModRef,
        }
    }

    fn get_mod_ref_info_with(
        &mut self,
        inst: llrm_mir::module::InstId,
        loc: Option<&MemoryLocation>,
        aaqi: &mut AAQueryInfo,
        others: &mut dyn FnMut(
            &MemoryLocation,
            &MemoryLocation,
            &mut AAQueryInfo,
            Option<llrm_mir::module::InstId>,
        ) -> AliasResult,
    ) -> ModRefInfo {
        match (&self.unit.function.instruction(inst).opcode, loc) {
            (Opcode::Call(_) | Opcode::Invoke(_), Some(loc)) => self.call_mod_ref(inst, loc, aaqi, Some(others)),
            _ => self.get_mod_ref_info(inst, loc, aaqi),
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/AliasAnalysis.cpp#L271
    /// `AAResults::getModRefInfo(Call1, Call2)` from the effects and argument
    /// attributes of the two calls. DEVIATE AA-D1: LLVM's reads the aggregate's
    /// answers; here they are BasicAA's own, which the aggregate meets with
    /// the others'. Exit: this moves into `AggregateAA`. Owner w-5, review
    /// 2026-11-19.
    fn get_mod_ref_info_call(
        &mut self,
        call1: llrm_mir::module::InstId,
        call2: llrm_mir::module::InstId,
        aaqi: &mut AAQueryInfo,
    ) -> ModRefInfo {
        use crate::aa::IRMemLocation::{ArgMem, InaccessibleMem};
        let is_call = |inst| matches!(
            self.unit.function.instruction(inst).opcode,
            Opcode::Call(_) | Opcode::Invoke(_)
        );
        if !is_call(call1) || !is_call(call2) {
            return ModRefInfo::ModRef;
        }
        let mut result = ModRefInfo::ModRef;
        // If either call is readnone, they do not interact.
        let call1_b = self.get_memory_effects(call1, aaqi);
        if call1_b.does_not_access_memory() {
            return ModRefInfo::NoModRef;
        }
        let call2_b = self.get_memory_effects(call2, aaqi);
        if call2_b.does_not_access_memory() {
            return ModRefInfo::NoModRef;
        }
        // If they both only read from memory, there is no dependence.
        if call1_b.only_reads_memory() && call2_b.only_reads_memory() {
            return ModRefInfo::NoModRef;
        }
        // If Call1 only reads memory, the only dependence on Call2 can be
        // from Call1 reading memory written by Call2.
        if call1_b.only_reads_memory() {
            result = result.intersect(ModRefInfo::Ref);
        } else if !call1_b.overall().is_ref_set() {
            result = result.intersect(ModRefInfo::Mod);
        }
        let argument_count = |call: llrm_mir::module::InstId, this: &Self| {
            this.unit.function.instruction(call).operands.len().saturating_sub(1)
        };

        // If Call2 only accesses memory through arguments, accumulate the
        // mod/ref information from Call1's references to the memory
        // referenced by Call2's arguments.
        if call2_b.only_accesses_arg_memory() {
            if call2_b.mod_ref(ArgMem).is_no_mod_ref() {
                return ModRefInfo::NoModRef;
            }
            let mut r = ModRefInfo::NoModRef;
            for at in 0..argument_count(call2, self) {
                if self.unit.space(self.unit.function.instruction(call2).operands[at]).is_none() {
                    continue;
                }
                let at = at as u32;
                let Some(place) = MemoryLocation::get_for_argument(self.unit, call2, at) else { continue };
                // What Call2 might do to its argument; Call1's dependence on
                // that location is the inverse.
                let arg_mod_ref_c2 = self.get_arg_mod_ref_info(call2, at);
                let mut arg_mask = ModRefInfo::NoModRef;
                if arg_mod_ref_c2.is_mod_set() {
                    arg_mask = ModRefInfo::ModRef;
                } else if arg_mod_ref_c2.is_ref_set() {
                    arg_mask = ModRefInfo::Mod;
                }
                arg_mask = arg_mask.intersect(self.call_mod_ref(call1, &place, aaqi, None));
                r = r.union(arg_mask).intersect(result);
                if r == result {
                    break;
                }
            }
            return r;
        }

        // If Call1 only accesses memory through arguments, check if Call2
        // references any of the memory referenced by Call1's arguments.
        if call1_b.only_accesses_arg_memory() {
            if call1_b.mod_ref(ArgMem).is_no_mod_ref() {
                return ModRefInfo::NoModRef;
            }
            let mut r = ModRefInfo::NoModRef;
            for at in 0..argument_count(call1, self) {
                if self.unit.space(self.unit.function.instruction(call1).operands[at]).is_none() {
                    continue;
                }
                let at = at as u32;
                let Some(place) = MemoryLocation::get_for_argument(self.unit, call1, at) else { continue };
                // If Call1 might Mod its argument, Call2's Mod or Ref matters;
                // if it might Ref, only a Mod by Call2.
                let arg_mod_ref_c1 = self.get_arg_mod_ref_info(call1, at);
                let mod_ref_c2 = self.call_mod_ref(call2, &place, aaqi, None);
                if (arg_mod_ref_c1.is_mod_set() && (mod_ref_c2.is_mod_set() || mod_ref_c2.is_ref_set()))
                    || (arg_mod_ref_c1.is_ref_set() && mod_ref_c2.is_mod_set())
                {
                    r = r.union(arg_mod_ref_c1).intersect(result);
                }
                if r == result {
                    break;
                }
            }
            return r;
        }

        // If only inaccessible memory has a set ModRefInfo, relate the same
        // locations.
        let only_inaccessible = |effects: crate::aa::MemoryEffects| {
            [ArgMem, crate::aa::IRMemLocation::ErrnoMem, crate::aa::IRMemLocation::Other]
                .into_iter()
                .all(|at| effects.mod_ref(at).is_no_mod_ref())
        };
        if only_inaccessible(call1_b) && only_inaccessible(call2_b) {
            let (use_mr, def_mr) = (call1_b.mod_ref(InaccessibleMem), call2_b.mod_ref(InaccessibleMem));
            if use_mr.is_no_mod_ref() || def_mr.is_no_mod_ref() || (def_mr == ModRefInfo::Ref && def_mr == use_mr) {
                return ModRefInfo::NoModRef;
            }
            return use_mr;
        }
        result
    }

    fn call_captures_before(
        &mut self,
        _: llrm_mir::module::InstId,
        _: &MemoryLocation,
        _: &mut AAQueryInfo,
    ) -> ModRefInfo {
        ModRefInfo::ModRef
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aa::IRMemLocation;
    use crate::testing::{function, layout, parsed, value};

    const M32: &str = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n";

    /// BasicAA's answer for the pointers `%a` and `%b` of `@f`, each accessed
    /// with `size` bytes.
    fn answer(
        body: &str,
        a: &str,
        b: &str,
        size: u64,
    ) -> AliasResult {
        let module = parsed(&format!("{M32}{body}"));
        let layout = layout(&module);
        let function = crate::testing::function(&module, "f");
        let unit = Unit::of(&module, &layout, function);
        let location =
            |name: &str| MemoryLocation::new(Operand::Value(value(function, name)), LocationSize::Precise(size));
        BasicAA::new(&unit).alias(&location(a), &location(b), &mut AAQueryInfo::default(), None)
    }

    /// Two inbounds geps off different bases that share a variable index: the
    /// index cancels, the constant left puts one access past the other's
    /// base object (`aliasGEP`'s inbounds rule). `opt -passes=aa-eval`
    /// (24.0.0git d1106deb71cc) says NoAlias for this text and MayAlias with
    /// the `inbounds` dropped; today's facts say MayAlias for both: 16,000
    /// of the pairs of the replay set.
    #[test]
    fn an_inbounds_gep_past_the_other_base_is_no_alias_and_a_plain_one_is_not() {
        let body = |gep: &str| {
            format!(
                "@g = internal global [820 x i8] zeroinitializer\ndefine i32 @f(ptr %p, i32 %i) {{\nentry:\n  %a = getelementptr {gep} i8, ptr @g, i32 %i\n  %b0 = getelementptr {gep} i8, ptr %p, i32 %i\n  %b = getelementptr {gep} i8, ptr %b0, i32 4\n  ret i32 0\n}}\n"
            )
        };
        assert_eq!(answer(&body("inbounds"), "a", "b", 4), AliasResult::NoAlias);
        assert_eq!(answer(&body(""), "a", "b", 4), AliasResult::MayAlias);
    }

    /// Two results of a callee that states its return `noalias` are distinct
    /// objects (`CallBase::hasRetAttr` reads the callee's attributes as well as
    /// the site's); with the attribute dropped they may be one. The pass read
    /// the site alone, so GVN re-read the second after a store through the
    /// first.
    #[test]
    fn two_results_of_a_callee_returning_noalias_are_no_alias() {
        let body = |attribute: &str| {
            format!(
                "declare {attribute}ptr @make()\ndefine i32 @f() {{\nentry:\n  %a = call ptr @make()\n  %b = call ptr @make()\n  ret i32 0\n}}\n"
            )
        };
        assert_eq!(answer(&body("noalias "), "a", "b", 4), AliasResult::NoAlias);
        assert_eq!(answer(&body(""), "a", "b", 4), AliasResult::MayAlias);
    }

    /// A pointer loaded from memory cannot be a local object whose address is
    /// never stored, passed or returned (`isEscapeSource` against a
    /// non-captured local); once the address is stored it can.
    #[test]
    fn a_loaded_pointer_is_no_alias_with_a_local_that_never_escapes() {
        let body = |escape: &str| {
            format!(
                "@cell = global ptr null\ndefine i32 @f(ptr %p) {{\nentry:\n  %b = alloca i32\n  {escape}\n  %a = load ptr, ptr @cell\n  ret i32 0\n}}\n"
            )
        };
        assert_eq!(answer(&body("store i32 0, ptr %b"), "a", "b", 4), AliasResult::NoAlias);
        assert_eq!(answer(&body("store ptr %b, ptr @cell"), "a", "b", 4), AliasResult::MayAlias);
    }

    /// The address of a local only compared keeps its provenance:
    /// `getCapturesBefore` asks for provenance only, so a loaded pointer is
    /// still apart from it. The check followed every capture and said MayAlias.
    #[test]
    fn a_local_whose_address_is_only_compared_is_apart_from_a_loaded_pointer() {
        let body = |address: &str| {
            format!(
                "@cell = global ptr null\ndefine i32 @f(ptr %p) {{\nentry:\n  %b = alloca i32\n  {address}\n  %a = load ptr, ptr @cell\n  ret i32 0\n}}\n"
            )
        };
        assert_eq!(answer(&body("%n = icmp eq ptr %b, %p"), "a", "b", 4), AliasResult::NoAlias);
        assert_eq!(answer(&body("store ptr %b, ptr @cell"), "a", "b", 4), AliasResult::MayAlias);
    }

    /// An object was classified (function-local, global, argument) again by
    /// every check of every query: 14 K calls for 3.3 K queries on d_faces -O2,
    /// 2.9 M instructions. A batch classifies each object once.
    #[test]
    fn a_batch_classifies_each_object_once() {
        let module = parsed(&format!(
            "{M32}define i32 @f(ptr %p) {{\nentry:\n  %a = alloca [16 x i8]\n  %b = alloca [16 x i8]\n  ret i32 0\n}}\n"
        ));
        let layout = layout(&module);
        let function = crate::testing::function(&module, "f");
        let unit = Unit::of(&module, &layout, function);
        let (a, b) = (Operand::Value(value(function, "a")), Operand::Value(value(function, "b")));
        let mut aa = BasicAA::new(&unit);
        let mut aaqi = AAQueryInfo::default();
        let before = CLASSIFICATIONS.with(std::cell::Cell::get);
        for size in 1..=10 {
            let (la, lb) = (
                MemoryLocation::new(a, LocationSize::Precise(size)),
                MemoryLocation::new(b, LocationSize::Precise(size)),
            );
            assert_eq!(aa.alias(&la, &lb, &mut aaqi, None), AliasResult::NoAlias);
        }
        let made = CLASSIFICATIONS.with(std::cell::Cell::get) - before;
        assert!(made <= 2, "{made} classifications of 2 objects in 10 queries");
    }

    /// A pointer was decomposed again by every query it was in (4.3 M
    /// instructions for 3 K queries on d_faces -O2). A batch decomposes each
    /// pointer once.
    #[test]
    fn a_batch_decomposes_each_pointer_once() {
        let geps: String =
            (0..6).map(|n| format!("  %g{n} = getelementptr inbounds i8, ptr %p, i32 {}\n", n * 8)).collect();
        let module = parsed(&format!("{M32}define i32 @f(ptr %p) {{\nentry:\n{geps}  ret i32 0\n}}\n"));
        let layout = layout(&module);
        let function = crate::testing::function(&module, "f");
        let unit = Unit::of(&module, &layout, function);
        let location =
            |name: &str| MemoryLocation::new(Operand::Value(value(function, name)), LocationSize::Precise(4));
        let mut aa = BasicAA::new(&unit);
        let mut aaqi = AAQueryInfo::default();
        let before = DECOMPOSITIONS.with(std::cell::Cell::get);
        for n in 1..6 {
            aa.alias(&location("g0"), &location(&format!("g{n}")), &mut aaqi, None);
        }
        let made = DECOMPOSITIONS.with(std::cell::Cell::get) - before;
        assert!(made <= 6, "{made} decompositions of 6 pointers in 5 queries");
    }

    /// CaptureTracking explores 150 uses
    /// (`capture-tracking-max-uses-to-explore`) and then takes the
    /// pointer as captured. The slot has two uses besides its loads; the pinned
    /// opt (24.0.0git d1106deb71cc) answers NoAlias with 148 loads and
    /// MayAlias with 149. A cap of 100 gave 32 MayAlias where LLVM said
    /// NoAlias on d_faces-O1 (an alloca with 123 uses).
    #[test]
    fn capture_tracking_gives_up_after_a_hundred_and_fifty_uses() {
        let body = |loads: usize| {
            let reads: String = (0..loads).map(|n| format!("  %w{n} = load ptr, ptr %slot\n")).collect();
            format!(
                "define i32 @f(ptr %p, i32 %i) {{\nentry:\n  %slot = alloca ptr\n  store ptr %p, ptr %slot\n  %q = load ptr, ptr %slot\n  %a = getelementptr inbounds i8, ptr %q, i32 %i\n{reads}  ret i32 0\n}}\n"
            )
        };
        assert_eq!(answer(&body(148), "a", "slot", 4), AliasResult::NoAlias);
        assert_eq!(answer(&body(149), "a", "slot", 4), AliasResult::MayAlias);
    }

    /// The call `%c` of `@f` in `body`: its effects and its argument 0's.
    fn call_effects(body: &str) -> (MemoryEffects, ModRefInfo) {
        let module = parsed(&format!("{M32}{body}"));
        let layout = layout(&module);
        let function = crate::testing::function(&module, "f");
        let unit = Unit::of(&module, &layout, function);
        let call = function
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| matches!(function.instruction(inst).opcode, Opcode::Call(_)))
            .expect("a call");
        let mut aa = BasicAA::new(&unit);
        (aa.get_memory_effects(call, &mut AAQueryInfo::default()), aa.get_arg_mod_ref_info(call, 0))
    }

    /// `getMemoryEffects(Call)` met the site's `memory(...)` with the callee's.
    /// The stub answered `unknown` for every call: `memory(argmem: read)` was
    /// `ModRef` everywhere, and `FunctionAttrs` could infer nothing.
    #[test]
    fn a_calls_effects_are_the_meet_of_its_site_and_its_callee() {
        use IRMemLocation::{ArgMem, Other};
        let (declared, _) = call_effects(
            "declare void @g(ptr) memory(argmem: read)\ndefine void @f(ptr %p) {\nentry:\n  call void @g(ptr %p)\n  ret void\n}\n",
        );
        assert_eq!(declared.mod_ref(ArgMem), ModRefInfo::Ref);
        assert_eq!(declared.mod_ref(Other), ModRefInfo::NoModRef);
        let (met, _) = call_effects(
            "declare void @g(ptr) memory(argmem: readwrite)\ndefine void @f(ptr %p) {\nentry:\n  call void @g(ptr %p) memory(argmem: write)\n  ret void\n}\n",
        );
        assert_eq!(met.mod_ref(ArgMem), ModRefInfo::Mod);
        let (none, _) =
            call_effects("declare void @g()\ndefine void @f() {\nentry:\n  call void @g()\n  ret void\n}\n");
        assert_eq!(none, MemoryEffects::unknown());
        let (readnone, _) =
            call_effects("declare void @g() readnone\ndefine void @f() {\nentry:\n  call void @g()\n  ret void\n}\n");
        assert!(readnone.does_not_access_memory());
    }

    /// `getArgModRefInfo`: `readnone`, `writeonly`, `readonly` on the site's
    /// argument or the callee's parameter, in that order.
    #[test]
    fn an_argument_is_what_its_attributes_leave_open() {
        let arg = |callee: &str, site: &str| {
            call_effects(&format!(
                "declare void @g(ptr {callee})\ndefine void @f(ptr %p) {{\nentry:\n  call void @g(ptr {site} %p)\n  ret void\n}}\n"
            ))
            .1
        };
        assert_eq!(arg("", ""), ModRefInfo::ModRef);
        assert_eq!(arg("readonly", ""), ModRefInfo::Ref);
        assert_eq!(arg("", "writeonly"), ModRefInfo::Mod);
        assert_eq!(arg("readonly", "readnone"), ModRefInfo::NoModRef);
    }

    /// Every argument query built the fact sets of the site's and the callee's
    /// attributes up to six times (`Facts::of` allocates each; 0.95 M Ir over
    /// d_faces' 2630 queries), where most arguments state none.
    #[test]
    fn an_argument_without_attributes_builds_no_facts() {
        let made = || ARGUMENT_FACTS.with(std::cell::Cell::get);
        let before = made();
        let body = "declare void @g(ptr)\ndefine void @f(ptr %p) {\nentry:\n  call void @g(ptr %p)\n  ret void\n}\n";
        call_effects(body);
        assert_eq!(made(), before, "facts built for an argument that states none");
        let body =
            "declare void @g(ptr readonly)\ndefine void @f(ptr %p) {\nentry:\n  call void @g(ptr %p)\n  ret void\n}\n";
        call_effects(body);
        assert_eq!(made(), before + 1, "one set for the callee's parameter, none for the site's");
    }

    /// Distinct allocas are distinct objects.
    #[test]
    fn two_allocas_are_no_alias_and_one_pointer_is_must_alias() {
        let body = "define i32 @f() {\nentry:\n  %a = alloca i32\n  %b = alloca i32\n  ret i32 0\n}\n";
        assert_eq!(answer(body, "a", "b", 4), AliasResult::NoAlias);
        assert_eq!(answer(body, "a", "a", 4), AliasResult::MustAlias);
    }

    /// `getModRefInfoMask` of `%a` in `@f`.
    fn mask(
        body: &str,
        ignore_locals: bool,
    ) -> ModRefInfo {
        let module = parsed(&format!("{M32}{body}"));
        let layout = layout(&module);
        let function = crate::testing::function(&module, "f");
        let unit = Unit::of(&module, &layout, function);
        let location = MemoryLocation::new(Operand::Value(value(function, "a")), LocationSize::Precise(4));
        BasicAA::new(&unit).get_mod_ref_info_mask(&location, &mut AAQueryInfo::default(), ignore_locals)
    }

    /// A pointer into a constant global is never written, one into a mutable
    /// global may be, a `noalias readonly` argument is read only (the
    /// mask answered ModRef for all of them before).
    #[test]
    fn the_mask_of_a_constant_global_is_none_and_of_a_mutable_one_is_everything() {
        let body = |kind: &str| {
            format!(
                "@g = internal {kind} [8 x i8] zeroinitializer\ndefine void @f() {{\nentry:\n  %a = getelementptr i8, ptr @g, i32 4\n  ret void\n}}\n"
            )
        };
        assert_eq!(mask(&body("constant"), false), ModRefInfo::NoModRef);
        assert_eq!(mask(&body("global"), false), ModRefInfo::ModRef);
        let argument = "define void @f(ptr noalias readonly %a) {\nentry:\n  ret void\n}\n";
        assert_eq!(mask(argument, false), ModRefInfo::Ref);
    }

    /// An alloca is skipped when locals are ignored and anything else when
    /// not: a select of a constant global and an alloca is NoModRef only with
    /// locals ignored.
    #[test]
    fn the_mask_skips_allocas_only_when_asked_and_follows_a_select() {
        let body = "@g = internal constant i32 0\ndefine void @f(i1 %c) {\nentry:\n  %l = alloca i32\n  %a = select i1 %c, ptr @g, ptr %l\n  ret void\n}\n";
        assert_eq!(mask(body, true), ModRefInfo::NoModRef);
        assert_eq!(mask(body, false), ModRefInfo::ModRef);
    }

    /// What the call in `@f` does to `%a`, the one call in the body.
    fn call_on(body: &str) -> ModRefInfo {
        let module = parsed(&format!("{M32}{body}"));
        let layout = layout(&module);
        let function = crate::testing::function(&module, "f");
        let unit = Unit::of(&module, &layout, function);
        let call = function
            .walk()
            .map(|(_, inst)| inst)
            .find(|inst| matches!(function.instruction(*inst).opcode, Opcode::Call(_)))
            .expect("a call");
        let location = MemoryLocation::new(Operand::Value(value(function, "a")), LocationSize::Precise(4));
        BasicAA::new(&unit).get_mod_ref_info(call, Some(&location), &mut AAQueryInfo::default())
    }

    /// What the first call does to the memory of the second, in the body of
    /// `@f`, with BasicAA alone (`getModRefInfo(Call1, Call2)`).
    fn pair_on(body: &str) -> ModRefInfo {
        let module = parsed(&format!("{M32}{body}"));
        let layout = layout(&module);
        let function = crate::testing::function(&module, "f");
        let unit = Unit::of(&module, &layout, function);
        let calls: Vec<_> = function
            .walk()
            .map(|(_, inst)| inst)
            .filter(|inst| matches!(function.instruction(*inst).opcode, Opcode::Call(_)))
            .collect();
        BasicAA::new(&unit).get_mod_ref_info_call(calls[0], calls[1], &mut AAQueryInfo::default())
    }

    /// Two calls, as the pinned `opt -passes=aa-eval -aa-pipeline=basic-aa
    /// -print-all-alias-modref-info` answers them (`AAResults::getModRefInfo(
    /// Call1, Call2)`). BasicAA said ModRef for every pair, so a client asking
    /// two read-only calls, or a `memory(none)` call and any other, saw a
    /// dependence LLVM does not.
    #[test]
    fn two_calls_are_related_by_their_effects_as_the_pin_relates_them() {
        let decls = "declare void @r1() memory(read)\ndeclare void @r2() memory(read)\ndeclare void @n() memory(none)\ndeclare void @any()\ndeclare void @w(ptr) memory(argmem: write)\ndeclare void @ir() memory(inaccessiblemem: read)\ndeclare void @irw() memory(inaccessiblemem: readwrite)\n";
        let f = |calls: &str| {
            format!("{decls}define void @f() {{\n  %a = alloca i32\n  %b = alloca i32\n{calls}  ret void\n}}\n")
        };
        // Both read: no dependence.
        assert_eq!(pair_on(&f("  call void @r1()\n  call void @r2()\n")), ModRefInfo::NoModRef);
        // readnone against anything.
        assert_eq!(pair_on(&f("  call void @n()\n  call void @any()\n")), ModRefInfo::NoModRef);
        // Argument memory only: apart slots, then the same slot.
        assert_eq!(pair_on(&f("  call void @w(ptr %a)\n  call void @w(ptr %b)\n")), ModRefInfo::NoModRef);
        assert_eq!(pair_on(&f("  call void @w(ptr %a)\n  call void @w(ptr %a)\n")), ModRefInfo::Mod);
        // Inaccessible memory only: the read against the write and back.
        assert_eq!(pair_on(&f("  call void @ir()\n  call void @irw()\n")), ModRefInfo::Ref);
        assert_eq!(pair_on(&f("  call void @irw()\n  call void @ir()\n")), ModRefInfo::ModRef);
        // bench_floats against report in floats-O2: an unknown callee may touch
        // inaccessible memory.
        assert_eq!(pair_on(&f("  call void @irw()\n  call void @any()\n")), ModRefInfo::ModRef);
    }

    /// A call touches a local slot nobody holds a pointer to only if the slot
    /// is passed; the call answered ModRef for both (the mod/ref methods
    /// were placeholders).
    #[test]
    fn a_call_cannot_touch_a_slot_that_never_escapes_unless_it_is_passed() {
        let body = |pass: &str| {
            format!(
                "declare void @h(ptr)\ndeclare void @g()\ndefine void @f() {{\nentry:\n  %a = alloca i32\n  call void {pass}\n  ret void\n}}\n"
            )
        };
        assert_eq!(call_on(&body("@g()")), ModRefInfo::NoModRef);
        assert_eq!(call_on(&body("@h(ptr %a)")), ModRefInfo::ModRef);
    }

    /// A `tail` call cannot reach the caller's slots, unless it takes one
    /// `byval`.
    #[test]
    fn a_tail_call_cannot_touch_a_slot_of_the_frame_unless_it_takes_one_by_value() {
        let body = |arg: &str| {
            format!(
                "declare void @h(ptr)\ndefine void @f() {{\nentry:\n  %a = alloca i32\n  tail call void @h(ptr {arg} %a)\n  ret void\n}}\n"
            )
        };
        assert_eq!(call_on(&body("")), ModRefInfo::NoModRef);
        assert_eq!(call_on(&body("byval(i32)")), ModRefInfo::ModRef);
    }

    /// A call that only reads through its argument reads the slot it is
    /// given, and touches no other (`getModRefInfo(Call, Loc)`'s argument
    /// refinement); with the effects unknown it was ModRef for both.
    #[test]
    fn a_call_that_reads_through_its_argument_only_reads_what_it_is_given() {
        let body = |pass: &str| {
            format!(
                "declare void @r(ptr) memory(argmem: read) nounwind\n@g = external global i32\ndefine void @f(ptr %q) {{\nentry:\n  %a = alloca i32\n  store ptr %a, ptr @g\n  call void @r(ptr {pass})\n  ret void\n}}\n"
            )
        };
        assert_eq!(call_on(&body("%a")), ModRefInfo::Ref);
        assert_eq!(call_on(&body("%q")), ModRefInfo::NoModRef);
    }

    /// On every load and store of the corpus that `MemRef` places at a constant
    /// offset in an alloca, `object_cell` finds the same object, offset and
    /// size: `slotstores` reads the second where it read the first.
    #[test]
    fn object_cell_agrees_with_memref_on_every_slot_access() {
        use crate::memory::MemRef;
        let (mut agreed, mut apart) = (0usize, Vec::new());
        for (name, module) in crate::testing::corpus() {
            let layout = layout(&module);
            for (_, global, function) in module.functions().filter(|(_, _, f)| !f.is_declaration()) {
                let unit = Unit::of(&module, &layout, function);
                for (_, inst) in function.walk() {
                    let Some(reference) = MemRef::of(&unit, inst) else { continue };
                    let Some(Operand::Value(slot)) = reference.root else { continue };
                    if !matches!(
                        unit.defining(Operand::Value(slot)).map(|(_, one)| &one.opcode),
                        Some(Opcode::Alloca { .. })
                    ) || reference.base.is_some()
                    {
                        continue;
                    }
                    let want = (Operand::Value(slot), reference.disp, u64::from(reference.width));
                    match object_cell(&unit, inst) {
                        Some(cell) if !cell.indexed && (cell.object, cell.offset, cell.size) == want => agreed += 1,
                        other => apart.push(format!(
                            "{name}/@{} {inst:?}: {want:?} vs {other:?}",
                            global.name.as_deref().unwrap_or("")
                        )),
                    }
                }
            }
        }
        // Premise: the corpus has slot accesses.
        assert!(agreed > 0, "no slot access in the corpus");
        assert!(
            apart.is_empty(),
            "{} of {} differ: {:#?}",
            apart.len(),
            agreed + apart.len(),
            &apart[..apart.len().min(8)]
        );
    }

    /// `isel` asks whether an access carries a `!tbaa` type: `AAInfo::tbaa`
    /// says it as `MemRef::typed` did, on every load and store of the
    /// corpus.
    #[test]
    fn a_tbaa_tag_is_present_where_memref_has_a_type() {
        use crate::memory::MemRef;
        let (mut tagged, mut differ) = (0usize, Vec::new());
        for (name, module) in crate::testing::corpus() {
            let layout = layout(&module);
            for (_, global, function) in module.functions().filter(|(_, _, f)| !f.is_declaration()) {
                let unit = Unit::of(&module, &layout, function);
                for (_, inst) in function.walk() {
                    let old = MemRef::of(&unit, inst).is_some_and(|one| one.typed.is_some());
                    let new = MemoryLocation::get(&unit, inst).is_some_and(|place| place.aa_info.tbaa.is_some());
                    tagged += usize::from(new);
                    if old != new {
                        differ
                            .push(format!("{name}/@{} {inst:?}: {old} vs {new}", global.name.as_deref().unwrap_or("")));
                    }
                }
            }
        }
        // Premise: the corpus has tagged accesses.
        assert!(tagged > 0, "no tagged access in the corpus");
        assert!(differ.is_empty(), "{} differ: {:#?}", differ.len(), &differ[..differ.len().min(8)]);
    }

    /// `DEF SEG = VARSEG(a)`: a far pointer made of `a`'s selector is
    /// `segment:0`, a pointer into the data segment, not into `a`; stripping
    /// the casts named `a` as its object and called it NoAlias with every
    /// other global.
    #[test]
    fn a_far_pointer_made_of_a_selector_is_may_alias_with_another_global() {
        let module = parsed(
            "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-ni:2\"

@pa = global [2 x i8] zeroinitializer
@pb = global [2 x i8] zeroinitializer

define i16 @f(i16 %n) {
b0:
  %near = getelementptr i8, ptr @pa, i16 0
  %far0 = addrspacecast ptr %near to ptr addrspace(1)
  %sg = addrspacecast ptr addrspace(1) %far0 to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %sg to ptr addrspace(1)
  %a = getelementptr i8, ptr addrspace(1) %far, i16 %n
  %b = getelementptr i8, ptr @pb, i16 0
  ret i16 0
}
",
        );
        let layout = layout(&module);
        let function = crate::testing::function(&module, "f");
        let unit = Unit::of(&module, &layout, function);
        let location =
            |name: &str| MemoryLocation::new(Operand::Value(value(function, name)), LocationSize::Precise(1));
        let answer = BasicAA::new(&unit).alias(&location("a"), &location("b"), &mut AAQueryInfo::default(), None);
        assert_eq!(answer, AliasResult::MayAlias);
    }

    /// lru.c: `addrspacecast (@c)` is a constant expression, not an
    /// instruction, and the cast was no base to look through: offset 8 of the
    /// far alias was may-alias with the near offset 12 (and not must-alias
    /// with the near offset 8), so a pointer stored at +8 was loaded again.
    #[test]
    fn a_far_cast_constant_of_a_global_is_the_global_at_its_offset() {
        let module = parsed(
            "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-ni:2\"

@c = internal global [48 x i8] zeroinitializer

define i16 @f(i16 %n) {
b0:
  %far = getelementptr i8, ptr addrspace(1) getelementptr inbounds nuw (i8, ptr addrspace(1) addrspacecast (ptr @c to ptr addrspace(1)), i16 8), i16 0
  %same = getelementptr i8, ptr getelementptr inbounds nuw (i8, ptr @c, i16 8), i16 0
  %next = getelementptr i8, ptr getelementptr inbounds nuw (i8, ptr @c, i16 12), i16 0
  ret i16 0
}
",
        );
        let layout = layout(&module);
        let function = crate::testing::function(&module, "f");
        let unit = Unit::of(&module, &layout, function);
        let location =
            |name: &str| MemoryLocation::new(Operand::Value(value(function, name)), LocationSize::Precise(4));
        let alias =
            |a: &str, b: &str| BasicAA::new(&unit).alias(&location(a), &location(b), &mut AAQueryInfo::default(), None);
        assert_eq!(alias("far", "same"), AliasResult::MustAlias);
        assert_eq!(alias("far", "next"), AliasResult::NoAlias);
    }

    /// `stripPointerCastsForAliasAnalysis` strips a zero-index `getelementptr`
    /// (instruction or constant expression), a single-arm phi and a far cast
    /// constant: each pointer is the object it came from.
    #[test]
    fn pointers_the_pin_strips_are_must_alias_with_their_source() {
        let module = parsed(
            "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-ni:2\"

@c = internal global [48 x i8] zeroinitializer

define i16 @f(i16 %n) {
b0:
  %p = getelementptr i8, ptr @c, i16 %n
  %zero = getelementptr i8, ptr %p, i16 0
  %zeroc = getelementptr i8, ptr getelementptr inbounds nuw (i8, ptr @c, i16 0), i16 0
  br label %b1
b1:
  %one = phi ptr [ %p, %b0 ]
  %far = getelementptr i8, ptr addrspace(1) addrspacecast (ptr @c to ptr addrspace(1)), i16 0
  %base = getelementptr i8, ptr @c, i16 0
  ret i16 0
}
",
        );
        let layout = layout(&module);
        let function = crate::testing::function(&module, "f");
        let unit = Unit::of(&module, &layout, function);
        let location =
            |name: &str| MemoryLocation::new(Operand::Value(value(function, name)), LocationSize::Precise(4));
        let alias =
            |a: &str, b: &str| BasicAA::new(&unit).alias(&location(a), &location(b), &mut AAQueryInfo::default(), None);
        assert_eq!(alias("zero", "p"), AliasResult::MustAlias);
        assert_eq!(alias("one", "p"), AliasResult::MustAlias);
        assert_eq!(alias("zeroc", "base"), AliasResult::MustAlias);
        assert_eq!(alias("far", "base"), AliasResult::MustAlias);
    }

    /// bintree.bas: an array element's far pointer is a segment word loaded
    /// from its descriptor, made a pointer by `inttoptr`. The pin takes it
    /// for a pointer that cannot be a local whose address never escaped; the
    /// first selector-cast fix stopped at the cast and lost that (a loop
    /// variable reloaded around each call, 14 B of the object). Where the
    /// module forges selectors it can be.
    #[test]
    fn a_far_pointer_from_a_segment_word_is_no_alias_with_a_local_unless_the_module_forges() {
        let module = parsed(
            "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-ni:2\"

declare void @g(ptr nocapture)

define i16 @f(i16 %seg, i16 %n) {
b0:
  %slot = alloca i16
  call void @g(ptr nocapture %slot)
  %sg = inttoptr i16 %seg to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %sg to ptr addrspace(1)
  %a = getelementptr i8, ptr addrspace(1) %far, i16 %n
  ret i16 0
}
",
        );
        let layout = layout(&module);
        let function = crate::testing::function(&module, "f");
        let answer = |forged: bool| {
            let mut globals = crate::globalsaa::Globals::default();
            globals.set_forged(forged);
            let unit = Unit::of(&module, &layout, function).with_globals_aa(&globals);
            let location =
                |name: &str| MemoryLocation::new(Operand::Value(value(function, name)), LocationSize::Precise(1));
            BasicAA::new(&unit).alias(&location("a"), &location("slot"), &mut AAQueryInfo::default(), None)
        };
        assert_eq!(answer(false), AliasResult::NoAlias);
        assert_eq!(answer(true), AliasResult::MayAlias);
    }

    /// `aliasPHI` and `aliasSelect`: a phi of loaded pointers and a select of
    /// locals against locals and arguments, the pairs the pin answers.
    const JOINS: &str = "define i16 @f(ptr %p, ptr %r, i1 %c, ptr %a1, ptr %a2) {
    b0:
      %s = alloca i16
      %t = alloca i16
      store i16 1, ptr %s
      %q1 = load ptr, ptr %p
      %q2 = load ptr, ptr %r
      br i1 %c, label %b1, label %b2

    b1:
      br label %b3

    b2:
      br label %b3

    b3:
      %q = phi ptr [ %q1, %b1 ], [ %q2, %b2 ]
      %sel = select i1 %c, ptr %s, ptr %t
      %sel2 = select i1 %c, ptr %a1, ptr %a2
      %sel3 = select i1 %c, ptr %s, ptr %a1
      ret i16 1
    }
    ";

    #[test]
    fn a_phi_and_a_select_are_apart_from_locals_as_the_pin_has_them() {
        let module = parsed(JOINS);
        let layout = layout(&module);
        let f = function(&module, "f");
        let unit = Unit::of(&module, &layout, f);
        let at = |name: &str| MemoryLocation::new(Operand::Value(value(f, name)), LocationSize::Precise(2));
        let mut aa = BasicAA::new(&unit);
        let mut ask = |one: &str, other: &str| aa.alias(&at(one), &at(other), &mut AAQueryInfo::default(), None);
        use AliasResult::{MayAlias, NoAlias};
        // The pin's answers.
        for (one, other, want) in [
            ("q", "s", NoAlias),
            ("q", "sel", NoAlias),
            ("s", "sel2", NoAlias),
            ("sel", "sel2", NoAlias),
            ("sel3", "t", NoAlias),
            ("sel2", "t", NoAlias),
            ("s", "sel", MayAlias),
            ("sel", "t", MayAlias),
            ("q", "sel2", MayAlias),
            ("q", "sel3", MayAlias),
            ("sel", "sel3", MayAlias),
        ] {
            assert_eq!(ask(one, other), want, "%{one} against %{other}");
            assert_eq!(ask(other, one), want, "%{other} against %{one}");
        }
    }

    /// Two pointers that step through the loop that holds them (each a phi
    /// with itself as an incoming value, through a `getelementptr`) ask each
    /// other: the pair is assumed apart while it is asked, and the answer is
    /// kept or dropped by what the sources prove. It recursed without end
    /// where the pair was not remembered until its answer was known.
    #[test]
    fn two_pointers_stepping_through_a_loop_are_apart_by_their_sources() {
        let module = parsed(
            "define void @f(ptr noalias %d, ptr noalias %s, i16 %n) {
b0:
  br label %b1

b1:
  %p = phi ptr [ %d, %b0 ], [ %p1, %b1 ]
  %q = phi ptr [ %s, %b0 ], [ %q1, %b1 ]
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
  %p1 = getelementptr i8, ptr %p, i16 1
  %q1 = getelementptr i8, ptr %q, i16 1
  %i1 = add i16 %i, 1
  %c = icmp ult i16 %i1, %n
  br i1 %c, label %b1, label %b2

b2:
  ret void
}
",
        );
        let layout = layout(&module);
        let f = function(&module, "f");
        let unit = Unit::of(&module, &layout, f);
        let at = |name: &str| MemoryLocation::new(Operand::Value(value(f, name)), LocationSize::Precise(1));
        let mut aa = BasicAA::new(&unit);
        let mut crossing = AAQueryInfo::default();
        crossing.may_be_cross_iteration = true;
        for (one, other) in [("p", "q"), ("p", "q1"), ("p1", "q")] {
            assert_eq!(aa.alias(&at(one), &at(other), &mut AAQueryInfo::default(), None), AliasResult::NoAlias);
            assert_eq!(aa.alias(&at(one), &at(other), &mut crossing, None), AliasResult::NoAlias);
        }
    }

    /// A value is the same value as itself only outside a cycle when two
    /// iterations may be compared (`isValueEqualInPotentialCycles`): a phi of
    /// a loop against itself is `MustAlias` within an iteration, `MayAlias`
    /// across them; the argument that has no loop in it stays `MustAlias`.
    #[test]
    fn a_value_in_a_loop_is_not_its_own_must_alias_across_iterations() {
        let module = parsed(
            "define void @f(ptr %a, i16 %n) {
b0:
  br label %b1

b1:
  %p = phi ptr [ %a, %b0 ], [ %p1, %b1 ]
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
  %p1 = getelementptr i8, ptr %p, i16 %n
  %i1 = add i16 %i, 1
  %c = icmp ult i16 %i1, %n
  br i1 %c, label %b1, label %b2

b2:
  ret void
}
",
        );
        let layout = layout(&module);
        let f = function(&module, "f");
        let unit = Unit::of(&module, &layout, f);
        let at = |name: &str| MemoryLocation::new(Operand::Value(value(f, name)), LocationSize::Precise(1));
        let mut aa = BasicAA::new(&unit);
        let mut crossing = AAQueryInfo::default();
        crossing.may_be_cross_iteration = true;
        assert_eq!(aa.alias(&at("p"), &at("p"), &mut AAQueryInfo::default(), None), AliasResult::MustAlias);
        assert_eq!(aa.alias(&at("p"), &at("p"), &mut crossing, None), AliasResult::MayAlias);
        assert_eq!(aa.alias(&at("a"), &at("a"), &mut crossing, None), AliasResult::MustAlias);
    }
}
