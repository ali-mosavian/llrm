//! `constraint-elimination`: conditions the dominating branches, assumes and
//! memory accesses already decide become constants, by Fourier-Motzkin
//! elimination on a system of linear constraints over the values compared.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ConstraintElimination.cpp`, each
//! function linking the lines it translates. Two [`ConstraintSystem`]s
//! (unsigned and signed) hold the facts that hold at the point being visited;
//! the dominator tree is walked in the order of its DFS numbers, a fact
//! entering the systems when the walk enters the block it holds in and leaving
//! when the walk leaves it (`DFSInStack`). A compare is turned into a row by
//! `decompose` (an offset plus coefficients of values, looking through `add`,
//! `sub`, `mul` and `shl` by a constant, `zext`, `sext`, `trunc` and `gep`),
//! and decided by asking the system whether the row or its negation is
//! implied.
//!
//! This file is the pass: the engine ([`Decomposition`], [`ConstraintTy`],
//! [`ConstraintInfo`]), the worklist over the dominator tree and the
//! [`ConstraintElimination`] function pass.
//!
//! DEVIATE (anything MIR lacks):
//! 1. A variable is an `Operand`: a value or a constant (a global, `null`),
//!    where LLVM's is a `Value *`.
//! 2. A constant-expression GEP is not a `GEPOperator` here: only a `gep`
//!    instruction is decomposed (`decomposeGEP`); a constant one is stripped to
//!    its base or stays a variable.
//! 3. `DebugCounter`, `DumpReproducers` (`generateReproducer`), the
//!    `LLVM_DEBUG` output and the debug-record rewrite of
//!    `replaceCmpWithConstant` are not translated.
//! 4. `takeName` has no counterpart: the `usub.sat` and overflow replacements
//!    get fresh names.
//! 5. Induction facts are in `constraintelim_induction`; every function of the
//!    pass is translated but the reproducer and the memory-access facts (6).
//! 6. `getBaseObjectSize` (`valuetracking::objectsize`) does not know the size
//!    of an allocation call, so no fact comes from an access into one.
//!
//! # Example
//! `if (x < 5) { if (x < 10) ... }`: the inner compare is decided true.
//!
//! # Used by
//! The pipeline's `constraint-elimination@699` slot, after Reassociate
//! (-O2, -O3, -Os; the pin's -O1 has none). `difference` and
//! `guards::Given::holds` have yet to move here.

use llrm_mir::constantrange::ConstantRange;
use llrm_mir::constraintsystem::{ConstraintSystem, Entry, Row};
use llrm_mir::context::signed;
use llrm_mir::hash::HashMap;
use llrm_mir::module::Operand;
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::known::{is_known_non_negative, is_known_positive};
use llrm_mir::valuetracking::objectsize::{ObjectSizeOpts, get_base_object_size};
use llrm_mir::valuetracking::pattern::{self, Side};
use llrm_mir::valuetracking::{MAX_ANALYSIS_RECURSION_DEPTH, Query};

use crate::constraintelim_induction::Inductions;

/// `MaxConstraintValue`.
const MAX_CONSTRAINT_VALUE: i64 = i64::MAX;
/// `MinSignedConstraintValue`.
const MIN_SIGNED_CONSTRAINT_VALUE: i64 = i64::MIN;

// ---- CmpInst ------------------------------------------------------------

/// `CmpInst::isSigned`.
pub fn is_signed(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
    )
}

/// `CmpInst::isUnsigned`.
pub fn is_unsigned(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Ugt | IntPredicate::Uge | IntPredicate::Ult | IntPredicate::Ule
    )
}

/// `CmpInst::isEquality`.
pub fn is_equality(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Eq | IntPredicate::Ne)
}

/// `ICmpInst::getSignedPredicate`.
pub fn signed_predicate(pred: IntPredicate) -> IntPredicate {
    match pred {
        IntPredicate::Ugt => IntPredicate::Sgt,
        IntPredicate::Uge => IntPredicate::Sge,
        IntPredicate::Ult => IntPredicate::Slt,
        IntPredicate::Ule => IntPredicate::Sle,
        other => other,
    }
}

/// `ICmpInst::getUnsignedPredicate`.
pub fn unsigned_predicate(pred: IntPredicate) -> IntPredicate {
    match pred {
        IntPredicate::Sgt => IntPredicate::Ugt,
        IntPredicate::Sge => IntPredicate::Uge,
        IntPredicate::Slt => IntPredicate::Ult,
        IntPredicate::Sle => IntPredicate::Ule,
        other => other,
    }
}

/// `CmpInst::getStrictPredicate`.
pub fn strict_predicate(pred: IntPredicate) -> IntPredicate {
    match pred {
        IntPredicate::Uge => IntPredicate::Ugt,
        IntPredicate::Ule => IntPredicate::Ult,
        IntPredicate::Sge => IntPredicate::Sgt,
        IntPredicate::Sle => IntPredicate::Slt,
        other => other,
    }
}

/// `CmpInst::getNonStrictPredicate`.
pub fn non_strict_predicate(pred: IntPredicate) -> IntPredicate {
    match pred {
        IntPredicate::Ugt => IntPredicate::Uge,
        IntPredicate::Ult => IntPredicate::Ule,
        IntPredicate::Sgt => IntPredicate::Sge,
        IntPredicate::Slt => IntPredicate::Sle,
        other => other,
    }
}

// ---- integer constants --------------------------------------------------

/// A `ConstantInt`: its bits, width and signed value.
#[derive(Clone, Copy, Debug)]
struct ConstInt {
    bits: u128,
    width: u32,
}

impl ConstInt {
    fn of(
        q: &Query,
        v: Operand,
    ) -> Option<Self> {
        q.int_constant(v).map(|(bits, width)| Self { bits, width })
    }

    fn sext(self) -> i128 {
        signed(self.bits, self.width)
    }

    fn is_negative(self) -> bool {
        self.sext() < 0
    }

    /// `canUseSExt`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L515>
    fn can_use_sext(self) -> bool {
        let value = self.sext();
        value > i128::from(MIN_SIGNED_CONSTRAINT_VALUE) && value < i128::from(MAX_CONSTRAINT_VALUE)
    }
}

// ---- decomposition ------------------------------------------------------

/// `DecompEntry`: a `Coefficient * Variable` entry after IR decomposition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecompEntry {
    pub coefficient: i64,
    pub variable: Side,
}

/// `Decomposition`: an `Offset + Coefficient1 * Variable1 + ...`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L309>
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decomposition {
    pub offset: i64,
    pub vars: Vec<DecompEntry>,
}

impl Decomposition {
    pub fn constant(offset: i64) -> Self {
        Self { offset, vars: Vec::new() }
    }

    pub fn variable(v: Side) -> Self {
        Self { offset: 0, vars: vec![DecompEntry { coefficient: 1, variable: v }] }
    }

    pub fn with(
        offset: i64,
        vars: Vec<DecompEntry>,
    ) -> Self {
        Self { offset, vars }
    }

    /// `add(int64_t)`: true if the operation overflows, i.e. the new
    /// decomposition is invalid.
    #[must_use]
    fn add_offset(
        &mut self,
        other: i64,
    ) -> bool {
        match self.offset.checked_add(other) {
            Some(sum) => {
                self.offset = sum;
                false
            }
            None => true,
        }
    }

    /// `add(const Decomposition &)`.
    #[must_use]
    pub fn add(
        &mut self,
        other: &Decomposition,
    ) -> bool {
        if self.add_offset(other.offset) {
            return true;
        }
        self.vars.extend_from_slice(&other.vars);
        false
    }

    /// `sub(const Decomposition &)`.
    #[must_use]
    pub fn sub(
        &mut self,
        other: &Decomposition,
    ) -> bool {
        let mut tmp = other.clone();
        if tmp.mul(-1) {
            return true;
        }
        if self.add_offset(tmp.offset) {
            return true;
        }
        self.vars.extend_from_slice(&tmp.vars);
        false
    }

    /// `mul(int64_t)`: every coefficient times `factor`; true on overflow.
    #[must_use]
    pub fn mul(
        &mut self,
        factor: i64,
    ) -> bool {
        match self.offset.checked_mul(factor) {
            Some(product) => self.offset = product,
            None => return true,
        }
        for var in &mut self.vars {
            match var.coefficient.checked_mul(factor) {
                Some(product) => var.coefficient = product,
                None => return true,
            }
        }
        false
    }
}

// ---- constraints --------------------------------------------------------

/// `ConstraintTy`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L266>
#[derive(Clone, Debug, Default)]
pub struct ConstraintTy {
    pub coefficients: Row,
    /// Number of variables the constraint is defined over.
    pub num_vars: usize,
    pub is_signed: bool,
    is_eq: bool,
    is_ne: bool,
}

impl ConstraintTy {
    pub fn new(
        coefficients: Row,
        num_vars: usize,
        is_signed: bool,
        is_eq: bool,
        is_ne: bool,
    ) -> Self {
        Self { coefficients, num_vars, is_signed, is_eq, is_ne }
    }

    pub fn is_empty(&self) -> bool {
        self.coefficients.is_empty()
    }

    pub fn is_eq(&self) -> bool {
        self.is_eq
    }

    pub fn is_ne(&self) -> bool {
        self.is_ne
    }

    /// `isImpliedBy`: true or false where the constraint is proven to be
    /// respectively true or false by `cs`, None where it is neither.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L973>
    pub fn is_implied_by(
        &self,
        cs: &ConstraintSystem<Side>,
    ) -> Option<bool> {
        let (sub_cs, new_coefficients) = cs.get_sub_system(&self.coefficients);
        let is_condition_implied = sub_cs.is_condition_implied(new_coefficients.clone());

        if self.is_eq || self.is_ne {
            let negated_or_equal = ConstraintSystem::<Side>::negate_or_equal(new_coefficients.clone());
            let is_negated_or_equal_implied =
                !negated_or_equal.is_empty() && sub_cs.is_condition_implied(negated_or_equal);

            // In order to check that `%a == %b` is true (equality), both
            // conditions `%a >= %b` and `%a <= %b` must hold true. When
            // checking for equality (`IsEq` is true), we return true if they
            // both hold, false in the other cases.
            if is_condition_implied && is_negated_or_equal_implied {
                return Some(self.is_eq);
            }

            let negated = ConstraintSystem::<Side>::negate(new_coefficients.clone());
            let is_negated_implied = !negated.is_empty() && sub_cs.is_condition_implied(negated);

            let strict_less_than = ConstraintSystem::<Side>::to_strict_less_than(new_coefficients);
            let is_strict_less_than_implied =
                !strict_less_than.is_empty() && sub_cs.is_condition_implied(strict_less_than);

            // In order to check that `%a != %b` is true (non-equality), either
            // condition `%a > %b` or `%a < %b` must hold true. When checking
            // for non-equality (`IsNe` is true), we return true if one of the
            // two holds, false in the other cases.
            if is_negated_implied || is_strict_less_than_implied {
                return Some(self.is_ne);
            }

            return None;
        }

        if is_condition_implied {
            return Some(true);
        }

        let negated = ConstraintSystem::<Side>::negate(new_coefficients);
        let is_negated_implied = !negated.is_empty() && sub_cs.is_condition_implied(negated);
        if is_negated_implied {
            return Some(false);
        }

        // Neither the condition nor its negated holds, did not prove anything.
        None
    }
}

/// `StackEntry`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L252>
#[derive(Clone, Debug)]
pub struct StackEntry {
    pub num_in: u32,
    pub num_out: u32,
    pub is_signed: bool,
    /// Variables that can be removed from the system once the stack entry gets
    /// removed.
    pub values_to_release: Vec<Side>,
}

// ---- GEP offsets --------------------------------------------------------

/// `OffsetResult`: variable and constant offsets for a chain of GEPs, with
/// base pointer `base_ptr`.
struct OffsetResult {
    base_ptr: Side,
    /// The constant offset, wrapped to the index width and signed.
    constant_offset: i64,
    /// The index width in bits.
    bit_width: u32,
    variable_offsets: Vec<(Side, i64)>,
    /// `nuw`, `nusw`, `inbounds` of the chain.
    flags: Flags,
}

/// The bits of `GEPNoWrapFlags` that matter here, of `flags`: `inbounds`
/// implies `nusw`, not `nuw`.
fn gep_flags(flags: Flags) -> Flags {
    let mut out = Flags::default();
    if flags.contains(Flags::INBOUNDS) {
        out.insert(Flags::INBOUNDS);
        out.insert(Flags::NUSW);
    }
    if flags.contains(Flags::NUSW) {
        out.insert(Flags::NUSW);
    }
    if flags.contains(Flags::NUW) {
        out.insert(Flags::NUW);
    }
    out
}

fn wrap_signed(
    value: i128,
    width: u32,
) -> i128 {
    signed(value as u128 & llrm_mir::context::mask(width), width)
}

/// `GEPOperator::collectOffset` for the `gep` instruction `inst`: None where
/// an offset cannot be collected (a struct index that is not a constant).
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Operator.cpp#L226>
fn collect_offset(
    q: &Query,
    source: llrm_mir::types::TypeId,
    operands: &[Operand],
    bit_width: u32,
) -> Option<(i64, Vec<(Side, i64)>)> {
    let indices: Vec<Option<i128>> =
        operands[1..].iter().map(|one| ConstInt::of(q, *one).map(ConstInt::sext)).collect();
    // A struct index must be a constant.
    let mut current = source;
    for (at, index) in indices.iter().enumerate() {
        if at == 0 {
            continue;
        }
        match q.context().types.get(current) {
            llrm_mir::types::Type::Array { element, .. } | llrm_mir::types::Type::Vector { element, .. } => {
                current = *element;
            }
            _ => {
                let field = (*index)?;
                current = q.context().types.member(current, field as u64)?;
            }
        }
    }
    let (constant, variable) = q.layout.collect_offset(&q.context().types, source, &indices);
    let constant = wrap_signed(constant, bit_width) as i64;
    let mut offsets: Vec<(Side, i64)> = Vec::new();
    for (at, scale) in variable {
        let scale = wrap_signed(i128::from(scale), bit_width) as i64;
        if scale == 0 {
            continue;
        }
        let index = Side::of(q, operands[1 + at]);
        match offsets.iter_mut().find(|(one, _)| *one == index) {
            Some((_, sum)) => *sum = wrap_signed(i128::from(*sum) + i128::from(scale), bit_width) as i64,
            None => offsets.push((index, scale)),
        }
    }
    Some((constant, offsets))
}

/// `collectOffsets`: variable and constant offsets for `gep` (an instruction),
/// partly traversing nested GEPs. None where collecting the offset fails.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L482>
fn collect_offsets(
    q: &Query,
    gep: Operand,
) -> Option<OffsetResult> {
    let (_, instruction) = q.instruction_of(gep)?;
    let Opcode::GetElementPtr { source } = instruction.opcode else { return None };
    let space = q
        .type_of(instruction.operands[0])
        .and_then(
            |ty| match q.context().types.get(ty) {
                llrm_mir::types::Type::Pointer(space) => Some(*space),
                _ => None,
            },
        )?;
    let bit_width = q.layout.offset_bits(space);
    let (mut constant, mut variable) = collect_offset(q, source, &instruction.operands, bit_width)?;
    let mut result = OffsetResult {
        base_ptr: Side::of(q, instruction.operands[0]),
        constant_offset: constant,
        bit_width,
        variable_offsets: Vec::new(),
        flags: gep_flags(instruction.flags),
    };

    // If we have a nested GEP, check if we can combine the constant offset of
    // the inner GEP with the outer GEP.
    if let Side::Value(base) = result.base_ptr
        && let Some((_, inner)) = q.instruction_of(base)
        && let Opcode::GetElementPtr { source: inner_source } = inner.opcode
    {
        let inner_collected = collect_offset(q, inner_source, &inner.operands, bit_width);
        // TODO in LLVM: Support cases with more than 1 variable offset.
        let can_collect_inner = inner_collected.is_some();
        let (constant2, variable2) = inner_collected.unwrap_or_default();
        if !can_collect_inner
            || variable.len() > 1
            || variable2.len() > 1
            || (!variable.is_empty() && !variable2.is_empty())
        {
            // More than 1 variable index, use outer result.
            result.variable_offsets = variable;
            return Some(result);
        }
        result.base_ptr = Side::of(q, inner.operands[0]);
        constant = wrap_signed(i128::from(constant) + i128::from(constant2), bit_width) as i64;
        result.constant_offset = constant;
        if variable.is_empty() && variable2.len() == 1 {
            variable = variable2;
        }
        result.flags = gep_flags(result.flags).intersect(gep_flags(inner.flags));
    }
    result.variable_offsets = variable;
    Some(result)
}

/// `GEPOperator::accumulateConstantOffset`: the offset of the `gep` `v`, if
/// every index is a constant.
pub(crate) fn constant_gep_offset(
    q: &Query,
    v: Operand,
) -> Option<i64> {
    let (_, instruction) = q.instruction_of(v)?;
    let Opcode::GetElementPtr { source } = instruction.opcode else { return None };
    let space = match q.context().types.get(q.type_of(instruction.operands[0])?) {
        llrm_mir::types::Type::Pointer(space) => *space,
        _ => return None,
    };
    let (constant, variable) = collect_offset(q, source, &instruction.operands, q.layout.offset_bits(space))?;
    variable.is_empty().then_some(constant)
}

/// `getConstraintFromMemoryAccess`: an access of `access_size` bytes through
/// `gep` with a variable index into an object of known size gives
/// `index <=u (size - access_size - constant offset) / scale`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L1412>
fn get_constraint_from_memory_access(
    q: &Query,
    gep: Operand,
    access_size: u64,
) -> Option<(CmpPredicate, Side, Side)> {
    let offset = collect_offsets(q, gep)?;
    if !offset.flags.contains(Flags::NUW) {
        return None;
    }
    if offset.variable_offsets.len() != 1 {
        return None;
    }
    let bit_width = offset.bit_width;
    let (index, scale) = offset.variable_offsets[0];
    // Bail out on non-canonical GEPs.
    if side_bits(q, index) != bit_width {
        return None;
    }
    let Side::Value(base) = offset.base_ptr else { return None };
    // Workaround for gep inbounds, ptr null, idx; be conservative since it is
    // not clear whether an out of bounds access to the padding is UB or not.
    let opts = ObjectSizeOpts { null_is_unknown_size: true, round_to_align: true };
    let size = get_base_object_size(q, base, opts)?;

    // Index * Scale + ConstOffset + AccessSize <= AllocSize. With nuw, the
    // index addition does not wrap: if (AllocSize - (ConstOffset +
    // AccessSize)) wraps around, there is no valid value for Index.
    let mask = llrm_mir::context::mask(bit_width);
    let wide = |value: i64| (i128::from(value) as u128) & mask;
    let max_index =
        (u128::from(size.wrapping_sub(access_size)) & mask).wrapping_sub(wide(offset.constant_offset)) & mask;
    let max_index = max_index / wide(scale);
    Some((CmpPredicate::new(IntPredicate::Ule), index, Side::Int(max_index, bit_width)))
}

// ---- sides --------------------------------------------------------------

/// The width of `s`'s integer type, a pointer's index width otherwise, 0 for
/// anything else.
fn side_bits(
    q: &Query,
    s: Side,
) -> u32 {
    match s {
        Side::Int(_, width) => width,
        Side::Value(v) => q.scalar_bits(v),
    }
}

/// Whether `s` is an integer (or a vector of them): `Type::isIntegerTy`.
pub(crate) fn is_integer(
    q: &Query,
    s: Side,
) -> bool {
    match s {
        Side::Int(..) => true,
        Side::Value(v) => {
            matches!(
                q.type_of(v).map(|ty| q.context().types.get(ty)),
                Some(llrm_mir::types::Type::Int(_))
            )
        }
    }
}

pub(crate) fn is_pointer(
    q: &Query,
    s: Side,
) -> bool {
    matches!(s, Side::Value(v) if q.is_pointer(v))
}

/// `m_Zero`.
fn is_zero(
    q: &Query,
    s: Side,
) -> bool {
    match s {
        Side::Int(bits, _) => bits == 0,
        Side::Value(v) => pattern::zero(q, v),
    }
}

/// `m_AllOnes`.
fn is_all_ones(
    q: &Query,
    s: Side,
) -> bool {
    matches!(
        s,
        Side::Int(bits, width) if bits == llrm_mir::context::mask(width)
    ) || matches!(
        s,
        Side::Value(v) if q.int_constant(v).is_some_and(|(bits, width)| bits == llrm_mir::context::mask(width))
    )
}

/// `ConstantInt::get(Ty, value)` for the type of `like`.
fn int_like(
    q: &Query,
    like: Side,
    value: i128,
) -> Side {
    let width = side_bits(q, like);
    Side::Int(value as u128 & llrm_mir::context::mask(width), width)
}

fn const_int(s: Side) -> Option<ConstInt> {
    match s {
        Side::Int(bits, width) => Some(ConstInt { bits, width }),
        Side::Value(_) => None,
    }
}

/// The instruction defining `s`, with the operand it defines.
fn defining<'a>(
    q: &Query<'a>,
    s: Side,
) -> Option<(Operand, llrm_mir::module::InstId, &'a llrm_mir::module::Instruction)> {
    let Side::Value(v) = s else { return None };
    let (inst, instruction) = q.instruction_of(v)?;
    Some((v, inst, instruction))
}

// ---- known no-wrap ------------------------------------------------------

/// `doesHoldInRange`: whether `info` implies that `op` is in `r`, interpreting
/// `r` as a signed range if `signed` is set and as an unsigned range
/// otherwise.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L522>
fn does_hold_in_range(
    info: &mut ConstraintInfo,
    q: &Query,
    op: Side,
    r: &ConstantRange,
    signed_range: bool,
) -> bool {
    if r.is_empty_set() || (if signed_range { r.is_sign_wrapped_set() } else { r.is_wrapped_set() }) {
        return false;
    }
    if r.is_full_set() {
        return true;
    }
    let width = r.width;
    let mut min = if signed_range { r.get_signed_min() } else { r.get_unsigned_min() };
    let mut max = if signed_range { r.get_signed_max() } else { r.get_unsigned_max() };
    let (min_val, max_val) = if signed_range {
        (1_u128 << (width - 1), llrm_mir::context::mask(width) >> 1)
    } else {
        (0, llrm_mir::context::mask(width))
    };
    // Replace bound too large to be decomposed by the largest usable one.
    if !signed_range && max >= MAX_CONSTRAINT_VALUE as u128 {
        max = MAX_CONSTRAINT_VALUE as u128 - 1;
    }
    min &= llrm_mir::context::mask(width);
    max &= llrm_mir::context::mask(width);
    let ty_like = op;
    if min != min_val
        && !info.does_hold(
            q,
            if signed_range { IntPredicate::Sge } else { IntPredicate::Uge },
            op,
            Side::Int(min, side_bits(q, ty_like)),
        )
    {
        return false;
    }
    if max != max_val
        && !info.does_hold(
            q,
            if signed_range { IntPredicate::Sle } else { IntPredicate::Ule },
            op,
            Side::Int(max, side_bits(q, ty_like)),
        )
    {
        return false;
    }
    true
}

/// `isKnownNoWrap(Opcode, Op0, Op1, NoWrapFlags, Info, Signed)`: whether
/// `opcode` applied to `op0` and `op1` with `flags` is known to not wrap in
/// signed or unsigned, depending on `signed_wrap`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L555>
fn is_known_no_wrap_op(
    opcode: BinaryOp,
    op0: Side,
    op1: Side,
    flags: Flags,
    info: &mut ConstraintInfo,
    q: &Query,
    signed_wrap: bool,
) -> bool {
    if flags.contains(if signed_wrap { Flags::NSW } else { Flags::NUW }) {
        return true;
    }
    if opcode == BinaryOp::Sub {
        // Op0 - Op1 does not wrap unsigned if Op0 >=u Op1.
        if !signed_wrap {
            return info.does_hold(q, IntPredicate::Uge, op0, op1);
        }
        // Op0 - Op1 does not wrap signed if 0 <=s Op1 <=s Op0.
        if info.is_known_non_negative(q, op1) && info.does_hold(q, IntPredicate::Sge, op0, op1) {
            return true;
        }
    }
    if !signed_wrap
        && flags.contains(Flags::NSW)
        && (opcode == BinaryOp::Shl || info.is_known_non_negative(q, op1))
        && info.is_known_non_negative(q, op0)
    {
        return true;
    }
    // For a constant Op1, the ranges of Op0 for which the operation does not
    // wrap are known exactly; check if the systems imply one of them.
    let Some(c) = const_int(op1) else { return false };
    let kind =
        if signed_wrap { llrm_mir::constantrange::NO_SIGNED_WRAP } else { llrm_mir::constantrange::NO_UNSIGNED_WRAP };
    let region = ConstantRange::make_exact_no_wrap_region(opcode, c.width, c.bits, kind);
    does_hold_in_range(info, q, op0, &region, signed_wrap)
}

/// `isKnownNoWrap(Value *V, Info, Signed)`: whether `v` is known to not wrap
/// in signed or unsigned, depending on `signed_wrap`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L594>
fn is_known_no_wrap(
    v: Side,
    info: &mut ConstraintInfo,
    q: &Query,
    signed_wrap: bool,
) -> bool {
    is_known_no_wrap_with(v, info, q, signed_wrap, None)
}

/// `isKnownNoWrap` of `v` as if it had `flags` (where given): LLVM's
/// `tryToStrengthenFlags` sets `nsw` on the instruction before it asks about
/// `nuw`, which reads it; here the instruction is edited after the query.
fn is_known_no_wrap_with(
    v: Side,
    info: &mut ConstraintInfo,
    q: &Query,
    signed_wrap: bool,
    flags: Option<Flags>,
) -> bool {
    let Side::Value(op) = v else { return false };
    if pattern::disjoint_or(q, op).is_some() {
        return true;
    }
    let Some((_, _, instruction)) = defining(q, v) else { return false };
    if let Some((_, call)) = pattern::with_overflow_inst(q, op)
        && let Some((call_inst, _)) = q.instruction_of(op)
        && let Some(llrm_mir::intrinsics::Intrinsic::WithOverflow { op: binary, .. }) = q.intrinsic(call_inst)
    {
        let args = &call.operands[..call.operands.len() - 1];
        return is_known_no_wrap_op(
            binary,
            Side::of(q, args[0]),
            Side::of(q, args[1]),
            Flags::default(),
            info,
            q,
            signed_wrap,
        );
    }
    if let Opcode::Cast(CastOp::Trunc) = instruction.opcode {
        if signed_wrap {
            return instruction.flags.contains(Flags::NSW);
        }
        // A trunc nsw only truncates without unsigned wrap if its operand is
        // non-negative.
        return instruction.flags.contains(Flags::NUW)
            || (instruction.flags.contains(Flags::NSW)
                && info.is_known_non_negative(q, Side::of(q, instruction.operands[0])));
    }
    let Opcode::Binary(opcode) = instruction.opcode else { return false };
    if !matches!(
        opcode,
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl
    ) {
        return false;
    }
    is_known_no_wrap_op(
        opcode,
        Side::of(q, instruction.operands[0]),
        Side::of(q, instruction.operands[1]),
        flags.unwrap_or(instruction.flags),
        info,
        q,
        signed_wrap,
    )
}

// ---- ConstraintInfo -----------------------------------------------------

/// `ConstraintInfo`: separate constraint systems and corresponding value
/// mappings for unsigned and signed information. Facts are added to and
/// conditions are checked against the corresponding system depending on the
/// signed-ness of their predicates. While the information is kept separate
/// based on signed-ness, certain conditions can be transferred between the two
/// systems.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L363>
pub struct ConstraintInfo {
    unsigned_cs: ConstraintSystem<Side>,
    signed_cs: ConstraintSystem<Side>,
    /// Decompositions computed against the current state of the systems. Must
    /// be cleared when the system changes.
    decompose_cache: HashMap<(Side, bool), Decomposition>,
}

impl ConstraintInfo {
    /// `ConstraintInfo(DL, FunctionArgs)`.
    pub fn new(function_args: &[Side]) -> Self {
        let mut info = Self {
            unsigned_cs: ConstraintSystem::with_arguments(function_args),
            signed_cs: ConstraintSystem::with_arguments(function_args),
            decompose_cache: HashMap::default(),
        };
        // Add Arg > -1 constraints to unsigned system for all function
        // arguments.
        for arg in function_args {
            let index = info.value2index(false)[arg];
            let size = info.value2index(false).len();
            info.unsigned_cs.add_row(&[Entry::new(0, 0), Entry::new(-1, index as u16)], size);
        }
        info
    }

    pub fn decompose_cache(&mut self) -> &mut HashMap<(Side, bool), Decomposition> {
        &mut self.decompose_cache
    }

    pub fn value2index(
        &self,
        signed: bool,
    ) -> &HashMap<Side, u32> {
        if signed { self.signed_cs.value2index() } else { self.unsigned_cs.value2index() }
    }

    pub fn value2index_mut(
        &mut self,
        signed: bool,
    ) -> &mut HashMap<Side, u32> {
        if signed { self.signed_cs.value2index_mut() } else { self.unsigned_cs.value2index_mut() }
    }

    pub fn cs(
        &self,
        signed: bool,
    ) -> &ConstraintSystem<Side> {
        if signed { &self.signed_cs } else { &self.unsigned_cs }
    }

    pub fn cs_mut(
        &mut self,
        signed: bool,
    ) -> &mut ConstraintSystem<Side> {
        if signed { &mut self.signed_cs } else { &mut self.unsigned_cs }
    }

    pub fn pop_last_constraint(
        &mut self,
        signed: bool,
    ) {
        assert!(self.decompose_cache.is_empty(), "Cache must be cleared");
        self.cs_mut(signed).pop_last_constraint();
    }

    pub fn pop_last_n_variables(
        &mut self,
        signed: bool,
        n: usize,
    ) {
        assert!(self.decompose_cache.is_empty(), "Cache must be cleared");
        self.cs_mut(signed).pop_last_n_variables(n);
    }

    /// `doesHold`: whether the systems imply `a pred b`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L1018>
    pub fn does_hold(
        &mut self,
        q: &Query,
        pred: IntPredicate,
        a: Side,
        b: Side,
    ) -> bool {
        let r = self.get_constraint_for_solving(q, pred, a, b);
        !r.is_empty() && self.cs(r.is_signed).is_condition_implied_in_sub_system(&r.coefficients)
    }

    /// `isKnownNonNegative`: whether `v` is known to be non-negative, either
    /// because the signed system implies it or because ValueTracking can prove
    /// it.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L1024>
    pub fn is_known_non_negative(
        &mut self,
        q: &Query,
        v: Side,
    ) -> bool {
        if let Some(ci) = const_int(v) {
            return !ci.is_negative();
        }
        let Side::Value(op) = v else { unreachable!("a constant is an Int") };
        is_known_non_negative(q, op, 0) || self.does_hold(q, IntPredicate::Sge, v, int_like(q, v, 0))
    }

    /// `isKnownPositive`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L1031>
    pub fn is_known_positive(
        &mut self,
        q: &Query,
        v: Side,
    ) -> bool {
        if let Some(ci) = const_int(v) {
            return ci.sext() > 0;
        }
        let Side::Value(op) = v else { unreachable!("a constant is an Int") };
        is_known_positive(q, op, 0) || self.does_hold(q, IntPredicate::Sgt, v, int_like(q, v, 0))
    }

    /// `decompose`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L691>
    pub fn decompose(
        &mut self,
        q: &Query,
        v: Side,
        is_signed: bool,
    ) -> Decomposition {
        if !may_look_through(q, v) {
            return self.decompose_impl(q, v, is_signed);
        }
        let key = (v, is_signed);
        if let Some(found) = self.decompose_cache.get(&key) {
            return found.clone();
        }
        let result = self.decompose_impl(q, v, is_signed);
        self.decompose_cache.insert(key, result.clone());
        result
    }

    /// `decomposeGEP`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L620>
    fn decompose_gep(
        &mut self,
        q: &Query,
        gep: Side,
        is_signed: bool,
    ) -> Decomposition {
        let Side::Value(gep_op) = gep else { return Decomposition::variable(gep) };
        let Some((_, _, instruction)) = defining(q, gep) else { return Decomposition::variable(gep) };
        // Do not reason about pointers where the index size is larger than 64
        // bits, as the coefficients used to encode constraints are 64 bit
        // integers.
        let base = instruction.operands[0];
        let space = match q.type_of(base).map(|ty| q.context().types.get(ty)) {
            Some(llrm_mir::types::Type::Pointer(space)) => *space,
            _ => return Decomposition::variable(gep),
        };
        // DEVIATE: a pointer whose offset carries into its selector (a huge
        // one) is not a base plus a sum.
        if q.layout.offset_bits(space) > 64 || q.layout.has_unstable_representation(space) {
            return Decomposition::variable(gep);
        }

        assert!(!is_signed, "The logic below only supports decomposition for unsigned predicates at the moment.");
        let Some(offsets) = collect_offsets(q, gep_op) else { return Decomposition::variable(gep) };
        let flags = offsets.flags;
        // We support either plain gep nuw, or gep nusw with non-negative
        // offset, which implies gep nuw.
        if flags == Flags::default() {
            return Decomposition::variable(gep);
        }

        // For a nuw-only GEP (nuw without nusw/inbounds), the offset must be
        // interpreted as unsigned.
        if !flags.contains(Flags::NUSW) && offsets.constant_offset < 0 {
            return Decomposition::variable(gep);
        }

        let mut result = Decomposition::with(
            offsets.constant_offset,
            vec![DecompEntry { coefficient: 1, variable: offsets.base_ptr }],
        );
        for (index, scale) in &offsets.variable_offsets {
            if !flags.contains(Flags::NUW) {
                // Try to prove nuw from nusw and nneg. If the index cannot be
                // proven non-negative, keep the GEP as-is instead of
                // decomposing it.
                debug_assert!(flags.contains(Flags::NUSW), "Must have nusw flag");
                if !self.is_known_non_negative(q, *index) {
                    return Decomposition::variable(gep);
                }
            }
            let mut idx_result = self.decompose(q, *index, is_signed);
            if idx_result.mul(*scale) {
                return Decomposition::variable(gep);
            }
            if result.add(&idx_result) {
                return Decomposition::variable(gep);
            }
        }
        let _ = offsets.bit_width;
        result
    }

    /// `decomposeImpl`: `v` as a constant offset plus pairs `{Coefficient,
    /// Variable}`, the sum of which equals `v`. Looking through certain
    /// expressions is only valid if a pre-condition holds, which is checked
    /// against the systems as needed.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L707>
    fn decompose_impl(
        &mut self,
        q: &Query,
        v: Side,
        is_signed: bool,
    ) -> Decomposition {
        let mut v = v;
        if is_pointer(q, v) && !is_signed {
            if matches!(
                v,
                Side::Value(op) if matches!(q.instruction_of(op).map(|(_, i)| i.opcode.clone()), Some(Opcode::GetElementPtr { .. }))
            ) {
                return self.decompose_gep(q, v, is_signed);
            }
            if matches!(v, Side::Value(op) if pattern::zero(q, op)) {
                return Decomposition::constant(0);
            }
            return Decomposition::variable(v);
        }

        // Don't handle integers > 64 bit. Our coefficients are 64-bit large,
        // so coefficient add/mul may wrap, while the operation in the full bit
        // width would not.
        if !is_integer(q, v) || side_bits(q, v) > 64 {
            return Decomposition::variable(v);
        }

        if let Some(ci) = const_int(v) {
            if is_signed {
                if ci.can_use_sext() {
                    return Decomposition::constant(ci.sext() as i64);
                }
            } else if ci.bits < MAX_CONSTRAINT_VALUE as u128 {
                return Decomposition::constant(ci.bits as i64);
            }
            return Decomposition::variable(v);
        }

        let ty_bits = side_bits(q, v);
        if let Some((_, _, instruction)) = defining(q, v) {
            match instruction.opcode {
                Opcode::Cast(CastOp::ZExt) => {
                    // In the signed system, the ZExt must be non-negative.
                    if is_signed && !instruction.flags.contains(Flags::NNEG) {
                        return Decomposition::variable(v);
                    }
                    v = Side::of(q, instruction.operands[0]);
                }
                Opcode::Cast(CastOp::SExt) => {
                    // In the unsigned system, the SExt operand must be
                    // non-negative.
                    let op0 = Side::of(q, instruction.operands[0]);
                    if !is_signed && !self.is_known_non_negative(q, op0) {
                        return Decomposition::variable(v);
                    }
                    v = op0;
                }
                Opcode::Cast(CastOp::Trunc) => {
                    let op0 = Side::of(q, instruction.operands[0]);
                    if side_bits(q, op0) <= 64 && is_known_no_wrap(v, self, q, is_signed) {
                        v = op0;
                    }
                }
                _ => {}
            }
        }

        let merge_results = |this: &mut Self, a: Side, b: Side, is_signed_b: bool| -> Option<Decomposition> {
            let mut res_a = this.decompose(q, a, is_signed);
            let res_b = this.decompose(q, b, is_signed_b);
            if res_a.add(&res_b) { None } else { Some(res_a) }
        };

        let Side::Value(op) = v else { return Decomposition::variable(v) };

        if let Some((op0, op1)) = pattern::add_like(q, op, Flags::default()) {
            let (op0, op1) = (Side::of(q, op0), Side::of(q, op1));
            if is_known_no_wrap(v, self, q, is_signed) {
                if let Some(decomp) = merge_results(self, op0, op1, is_signed) {
                    return decomp;
                }
                return Decomposition::variable(v);
            }
            // In the unsigned system, adding a negative constant only wraps if
            // Op0 is smaller than it.
            if !is_signed
                && let Some(ci) = const_int(op1)
                && ci.is_negative()
                && ci.can_use_sext()
                && self.does_hold(q, IntPredicate::Uge, op0, int_like(q, op0, -ci.sext()))
                && let Some(decomp) = merge_results(self, op0, op1, true)
            {
                return decomp;
            }
            return Decomposition::variable(v);
        }

        // `xor %x, -1` is equivalent to `sub nsw -1, %x`.
        if is_signed && let Some(op0) = pattern::not(q, op) {
            let mut result = Decomposition::constant(-1);
            let inner = self.decompose(q, Side::of(q, op0), is_signed);
            if !result.sub(&inner) {
                return result;
            }
            return Decomposition::variable(v);
        }

        if let Some((_, _, op0, op1)) = pattern::binary(q, op, BinaryOp::Sub) {
            let (op0, op1) = (Side::of(q, op0), Side::of(q, op1));
            if is_known_no_wrap(v, self, q, is_signed) {
                let mut res_a = self.decompose(q, op0, is_signed);
                let res_b = self.decompose(q, op1, is_signed);
                if !res_a.sub(&res_b) {
                    return res_a;
                }
            }
            return Decomposition::variable(v);
        }

        if let Some((_, _, op0, op1)) = pattern::binary(q, op, BinaryOp::Mul)
            && let Some(ci) = const_int(Side::of(q, op1))
        {
            // A negative constant is only a valid coefficient in the signed
            // system; in the unsigned system the multiplier is the constant's
            // unsigned value.
            if ci.can_use_sext() && (is_signed || !ci.is_negative()) && is_known_no_wrap(v, self, q, is_signed) {
                let mut result = self.decompose(q, Side::of(q, op0), is_signed);
                if !result.mul(ci.sext() as i64) {
                    return result;
                }
            }
            return Decomposition::variable(v);
        }

        if let Some((_, _, op0, op1)) = pattern::binary(q, op, BinaryOp::Shl)
            && let Some(ci) = const_int(Side::of(q, op1))
        {
            // (shl x, shift) is (mul x, 1 << shift). The scale must fit in the
            // signed coefficient, so reject shifts >= 63. Also reject a shift
            // of bw-1, for which the product is not representable.
            let max_shift = if is_signed { i128::from(ty_bits) - 1 } else { 63 };
            if !ci.is_negative() && ci.sext() < max_shift && is_known_no_wrap(v, self, q, is_signed) {
                let mut result = self.decompose(q, Side::of(q, op0), is_signed);
                if !result.mul(1_i64 << ci.sext()) {
                    return result;
                }
            }
            return Decomposition::variable(v);
        }

        Decomposition::variable(v)
    }

    /// `getConstraint`: a comparison `op0 pred op1` as a constraint, using
    /// indices from the corresponding constraint system. New variables that
    /// need to be added to the system are collected in `new_variables`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L878>
    pub fn get_constraint(
        &mut self,
        q: &Query,
        pred: IntPredicate,
        op0: Side,
        op1: Side,
        new_variables: &mut Vec<Side>,
        force_signed_system: bool,
    ) -> ConstraintTy {
        assert!(new_variables.is_empty(), "NewVariables must be empty when passed in");
        assert!(!force_signed_system || is_equality(pred), "signed system can only be forced on eq/ne");

        let (mut pred, mut op0, mut op1) = (pred, op0, op1);
        let (mut is_eq, mut is_ne) = (false, false);

        // Try to convert Pred to one of ULE/ULT/SLE/SLT.
        match pred {
            IntPredicate::Ugt | IntPredicate::Uge | IntPredicate::Sgt | IntPredicate::Sge => {
                pred = pred.swapped();
                std::mem::swap(&mut op0, &mut op1);
            }
            IntPredicate::Eq => {
                if !force_signed_system && is_zero(q, op1) {
                    pred = IntPredicate::Ule;
                } else {
                    is_eq = true;
                    pred = IntPredicate::Ule;
                }
            }
            IntPredicate::Ne => {
                if !force_signed_system && is_zero(q, op1) {
                    pred = IntPredicate::Ugt.swapped();
                    std::mem::swap(&mut op0, &mut op1);
                } else {
                    is_ne = true;
                    pred = IntPredicate::Ule;
                }
            }
            _ => {}
        }

        if !matches!(
            pred,
            IntPredicate::Ule | IntPredicate::Ult | IntPredicate::Sle | IntPredicate::Slt
        ) {
            return ConstraintTy::default();
        }

        let is_signed_system = force_signed_system || is_signed(pred);
        let a_dec = self.decompose(q, strip_pointer_casts_same_representation(q, op0), is_signed_system);
        let b_dec = self.decompose(q, strip_pointer_casts_same_representation(q, op1), is_signed_system);
        let mut r = get_row_for_less_equal(&a_dec, &b_dec, self.value2index(is_signed_system), new_variables);
        if r.is_empty() {
            return ConstraintTy::default();
        }

        if matches!(pred, IntPredicate::Slt | IntPredicate::Ult) {
            match r[0].coefficient.checked_add(-1) {
                Some(constant) => r[0].coefficient = constant,
                None => return ConstraintTy::default(),
            }
        }

        // Remove any new variable without a coefficient in the row.
        let num_v2i = self.value2index(is_signed_system).len();
        let last_id = r[r.len() - 1].id as usize;
        new_variables.truncate(if last_id > num_v2i { last_id - num_v2i } else { 0 });

        ConstraintTy::new(r, num_v2i + new_variables.len(), is_signed_system, is_eq, is_ne)
    }

    /// `getConstraintForSolving`: `op0 pred op1` as a constraint using
    /// `get_constraint`; empty if the result cannot be used to query the
    /// existing constraint system, e.g. because it would require adding new
    /// variables. Also tries to convert signed predicates to unsigned ones if
    /// possible to allow using the unsigned system which increases the
    /// effectiveness of the signed <-> unsigned transfer logic.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L945>
    pub fn get_constraint_for_solving(
        &mut self,
        q: &Query,
        pred: IntPredicate,
        op0: Side,
        op1: Side,
    ) -> ConstraintTy {
        let mut pred = pred;
        // Handle trivially true compares directly to avoid adding V UGE 0
        // constraints for all variables in the unsigned system.
        if (pred == IntPredicate::Ule && is_zero(q, op0)) || (pred == IntPredicate::Uge && is_zero(q, op1)) {
            // Return constraint that's trivially true.
            return ConstraintTy::new(vec![Entry::new(0, 0)], 0, false, false, false);
        }

        // If both operands are known to be non-negative, change signed
        // predicates to unsigned ones. This increases the reasoning
        // effectiveness in combination with the signed <-> unsigned transfer
        // logic.
        if is_signed(pred)
            && side_is_known_non_negative(q, op0, MAX_ANALYSIS_RECURSION_DEPTH - 1)
            && side_is_known_non_negative(q, op1, MAX_ANALYSIS_RECURSION_DEPTH - 1)
        {
            pred = unsigned_predicate(pred);
        }

        let mut new_variables = Vec::new();
        let r = self.get_constraint(q, pred, op0, op1, &mut new_variables, false);
        if !new_variables.is_empty() {
            return ConstraintTy::default();
        }
        r
    }

    /// `transferToOtherSystem`: tries to add information from `a pred b` to
    /// the unsigned/signed system if `pred` is signed/unsigned.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L1038>
    pub fn transfer_to_other_system(
        &mut self,
        q: &Query,
        pred: IntPredicate,
        a: Side,
        b: Side,
        num_in: u32,
        num_out: u32,
        dfs_in_stack: &mut Vec<StackEntry>,
    ) {
        // Check if we can combine facts from the signed and unsigned systems
        // to derive additional facts.
        if !is_integer(q, a) {
            return;
        }
        // FIXME in LLVM: This currently depends on the order we add facts.
        // Ideally we would first add all known facts and only then try to add
        // additional facts.
        match pred {
            IntPredicate::Ult | IntPredicate::Ule => {
                // If B is a signed positive constant, then A >=s 0 and A <s
                // (or <=s) B.
                if self.is_known_non_negative(q, b) {
                    self.add_fact(q, IntPredicate::Sge, a, int_like(q, b, 0), num_in, num_out, dfs_in_stack);
                    self.add_fact(q, signed_predicate(pred), a, b, num_in, num_out, dfs_in_stack);
                }
            }
            IntPredicate::Uge | IntPredicate::Ugt => {
                // If A is a signed positive constant, then B >=s 0 and A >s
                // (or >=s) B.
                if self.is_known_non_negative(q, a) {
                    self.add_fact(q, IntPredicate::Sge, b, int_like(q, b, 0), num_in, num_out, dfs_in_stack);
                    self.add_fact(q, signed_predicate(pred), a, b, num_in, num_out, dfs_in_stack);
                }
            }
            IntPredicate::Slt | IntPredicate::Sle => {
                if self.is_known_non_negative(q, a) {
                    self.add_fact(q, unsigned_predicate(pred), a, b, num_in, num_out, dfs_in_stack);
                }
            }
            IntPredicate::Sgt => {
                if self.does_hold(q, IntPredicate::Sge, b, int_like(q, b, -1)) {
                    self.add_fact(q, IntPredicate::Uge, a, int_like(q, b, 0), num_in, num_out, dfs_in_stack);
                }
                if self.is_known_non_negative(q, b) {
                    self.add_fact(q, IntPredicate::Ugt, a, b, num_in, num_out, dfs_in_stack);
                }
            }
            IntPredicate::Sge => {
                if self.is_known_non_negative(q, b) {
                    self.add_fact(q, IntPredicate::Uge, a, b, num_in, num_out, dfs_in_stack);
                }
            }
            _ => {}
        }
    }

    /// `addFact`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L2201>
    pub fn add_fact(
        &mut self,
        q: &Query,
        pred: IntPredicate,
        a: Side,
        b: Side,
        num_in: u32,
        num_out: u32,
        dfs_in_stack: &mut Vec<StackEntry>,
    ) {
        self.add_fact_impl(q, pred, a, b, num_in, num_out, dfs_in_stack, false);
        // If the Pred is eq/ne, also add the fact to signed system.
        if is_equality(pred) {
            self.add_fact_impl(q, pred, a, b, num_in, num_out, dfs_in_stack, true);
        }
        if pred == IntPredicate::Ne {
            self.tighten_bound_using_ne(q, a, b, num_in, num_out, dfs_in_stack);
        }
    }

    /// `tightenBoundUsingNe`: tries to use the inequality `a != b` to tighten
    /// a non-strict bound the system already implies to the corresponding
    /// strict bound.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L2212>
    fn tighten_bound_using_ne(
        &mut self,
        q: &Query,
        a: Side,
        b: Side,
        num_in: u32,
        num_out: u32,
        dfs_in_stack: &mut Vec<StackEntry>,
    ) {
        if !(is_integer(q, a) || is_pointer(q, a)) {
            return;
        }
        for is_signed_system in [false, true] {
            // In the unsigned system `A u>= 0` holds for every A, so
            // getConstraint already turned `A != 0` into `A u> 0`.
            if !is_signed_system && is_zero(q, b) {
                continue;
            }
            // Skip if there are any unknown variables.
            let decomposed = self.decompose(q, a, is_signed_system);
            let value2index = self.value2index(is_signed_system);
            if decomposed.vars.iter().any(|entry| !value2index.contains_key(&entry.variable)) {
                continue;
            }
            // If the system implies `A >= B` then together with `A != B` we
            // get the strict `A > B`; symmetrically `A <= B` becomes `A < B`.
            let (ge_pred, le_pred) = if is_signed_system {
                (IntPredicate::Sge, IntPredicate::Sle)
            } else {
                (IntPredicate::Uge, IntPredicate::Ule)
            };
            for non_strict in [ge_pred, le_pred] {
                if !self.does_hold(q, non_strict, a, b) {
                    continue;
                }
                let strict = strict_predicate(non_strict);
                self.add_fact_impl(q, strict, a, b, num_in, num_out, dfs_in_stack, false);
                break;
            }
        }
    }

    /// `addFactImpl`: adds facts into the constraint system.
    /// `force_signed_system` can be set when `pred` is eq/ne, and the signed
    /// constraint system is used when it's specified.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L2253>
    fn add_fact_impl(
        &mut self,
        q: &Query,
        pred: IntPredicate,
        a: Side,
        b: Side,
        num_in: u32,
        num_out: u32,
        dfs_in_stack: &mut Vec<StackEntry>,
        force_signed_system: bool,
    ) {
        let mut new_variables = Vec::new();
        let mut r = self.get_constraint(q, pred, a, b, &mut new_variables, force_signed_system);

        // TODO in LLVM: Support non-equality for facts as well.
        if r.is_empty() || r.is_ne() {
            return;
        }

        let added = self.cs_mut(r.is_signed).add_row(&r.coefficients, r.num_vars);
        if !added {
            return;
        }

        self.decompose_cache.clear();

        // If R has been added to the system, add the new variables and queue
        // it for removal once it goes out-of-scope.
        let mut values_to_release = Vec::new();
        for v in &new_variables {
            let value2index = self.value2index_mut(r.is_signed);
            let next = value2index.len() as u32 + 1;
            value2index.entry(*v).or_insert(next);
            values_to_release.push(*v);
        }

        dfs_in_stack.push(StackEntry { num_in, num_out, is_signed: r.is_signed, values_to_release });

        if !r.is_signed {
            for v in &new_variables {
                // Add V > -1 constraints for all new variables.
                let index = self.value2index(false)[v];
                let size = self.value2index(false).len();
                self.cs_mut(false).add_row(&[Entry::new(0, 0), Entry::new(-1, index as u16)], size);
                dfs_in_stack.push(StackEntry {
                    num_in,
                    num_out,
                    is_signed: r.is_signed,
                    values_to_release: Vec::new(),
                });
            }
        }

        if r.is_eq() {
            // Also add the inverted constraint for equality constraints.
            for entry in &mut r.coefficients {
                match entry.coefficient.checked_mul(-1) {
                    Some(negated) => entry.coefficient = negated,
                    None => return,
                }
            }
            self.cs_mut(r.is_signed).add_row(&r.coefficients, r.num_vars);

            dfs_in_stack.push(StackEntry { num_in, num_out, is_signed: r.is_signed, values_to_release: Vec::new() });
        }
    }

    /// `removeEntryFromStack`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L2112>
    pub fn remove_entry_from_stack(
        &mut self,
        entry: &StackEntry,
        dfs_in_stack: &mut Vec<StackEntry>,
    ) {
        self.decompose_cache.clear();
        self.pop_last_constraint(entry.is_signed);
        // Remove variables in the system that went out of scope.
        let mapping = self.value2index_mut(entry.is_signed);
        for v in &entry.values_to_release {
            mapping.remove(v);
        }
        self.pop_last_n_variables(entry.is_signed, entry.values_to_release.len());
        dfs_in_stack.pop();
    }
}

/// `isKnownNonNegative(Op, DL, Depth)` of ValueTracking, of a side.
fn side_is_known_non_negative(
    q: &Query,
    s: Side,
    depth: u32,
) -> bool {
    match s {
        Side::Int(bits, width) => signed(bits, width) >= 0,
        Side::Value(op) => is_known_non_negative(q, op, depth),
    }
}

/// `Value::stripPointerCastsSameRepresentation`: `v` without the pointer casts
/// and the GEPs with all indices zero around it.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Value.cpp#L642>
fn strip_pointer_casts_same_representation(
    q: &Query,
    v: Side,
) -> Side {
    let Side::Value(mut op) = v else { return v };
    if !q.is_pointer(op) {
        return v;
    }
    // Even though we don't look through PHI nodes, we could be called on an
    // instruction in an unreachable block, which may be on a cycle.
    let mut visited = vec![op];
    loop {
        let next = match op {
            Operand::Value(_) => {
                let Some((_, instruction)) = q.instruction_of(op) else { return Side::of(q, op) };
                match instruction.opcode {
                    Opcode::GetElementPtr { .. } => {
                        if !instruction.operands[1..]
                            .iter()
                            .all(|index| q.int_constant(*index).is_some_and(|(bits, _)| bits == 0))
                        {
                            return Side::of(q, op);
                        }
                        instruction.operands[0]
                    }
                    Opcode::Cast(CastOp::BitCast) => {
                        if !q.is_pointer(instruction.operands[0]) {
                            return Side::of(q, op);
                        }
                        instruction.operands[0]
                    }
                    Opcode::Call(_) => match pattern::returned_argument_operand(q, instruction) {
                        Some(returned) => returned,
                        None => return Side::of(q, op),
                    },
                    _ => return Side::of(q, op),
                }
            }
            Operand::Constant(id) => match &q.context().get(id).kind {
                llrm_mir::context::ConstantKind::Expr(llrm_mir::context::ConstantExpr::GetElementPtr {
                    operands,
                    ..
                }) if operands[1..]
                    .iter()
                    .all(|index| q.int_constant(Operand::Constant(*index)).is_some_and(|(bits, _)| bits == 0)) =>
                {
                    Operand::Constant(operands[0])
                }
                llrm_mir::context::ConstantKind::Expr(llrm_mir::context::ConstantExpr::Cast {
                    op: CastOp::BitCast,
                    value,
                }) => Operand::Constant(*value),
                _ => return Side::of(q, op),
            },
            Operand::Block(_) => return Side::of(q, op),
        };
        op = next;
        if visited.contains(&op) {
            return Side::of(q, op);
        }
        visited.push(op);
    }
}

/// `mayLookThrough`: whether `v` is an operation `decompose_impl` can look
/// through.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L670>
fn may_look_through(
    q: &Query,
    v: Side,
) -> bool {
    let Some((_, _, instruction)) = defining(q, v) else { return false };
    matches!(
        instruction.opcode,
        Opcode::GetElementPtr { .. }
            | Opcode::Binary(
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl | BinaryOp::Or | BinaryOp::Xor
            )
            | Opcode::Cast(CastOp::ZExt | CastOp::SExt | CastOp::Trunc)
    )
}

/// `getRowForLessEqual`: the row for `a_dec <= b_dec`, using the indices from
/// `value2index`. Variables not in `value2index` are appended to
/// `new_variables` and get the indices following the ones in `value2index`.
/// Empty if the coefficients overflow.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L833>
fn get_row_for_less_equal(
    a_dec: &Decomposition,
    b_dec: &Decomposition,
    value2index: &HashMap<Side, u32>,
    new_variables: &mut Vec<Side>,
) -> Row {
    // Build the row, by first adding all coefficients from A and then
    // subtracting all coefficients from B.
    let Some(offset_sum) = b_dec.offset.checked_sub(a_dec.offset) else { return Row::new() };
    let mut r: Row = vec![Entry::new(offset_sum, 0)];
    // The entry for Idx, or the place to insert it at, is the first entry with
    // an index >= Idx.
    fn coefficient(
        r: &mut Row,
        idx: u16,
    ) -> &mut i64 {
        let at = r.iter().skip(1).position(|e| e.id >= idx).map_or(r.len(), |p| p + 1);
        if at == r.len() || r[at].id != idx {
            r.insert(at, Entry::new(0, idx));
        }
        &mut r[at].coefficient
    }
    // First try to look up V in Value2Index and NewVariables. Otherwise add a
    // new entry to NewVariables.
    let mut get_or_add_index = |v: Side| -> u16 {
        if let Some(index) = value2index.get(&v) {
            return *index as u16;
        }
        let at = match new_variables.iter().position(|one| *one == v) {
            Some(at) => at,
            None => {
                new_variables.push(v);
                new_variables.len() - 1
            }
        };
        (value2index.len() + at + 1) as u16
    };
    for kv in &a_dec.vars {
        let idx = get_or_add_index(kv.variable);
        *coefficient(&mut r, idx) += kv.coefficient;
    }
    for kv in &b_dec.vars {
        let idx = get_or_add_index(kv.variable);
        let coeff = coefficient(&mut r, idx);
        match coeff.checked_sub(kv.coefficient) {
            Some(difference) => *coeff = difference,
            None => return Row::new(),
        }
    }
    // Drop coefficients that cancelled out.
    r.retain(|e| !(e.id != 0 && e.coefficient == 0));
    r
}

// ---- checks -------------------------------------------------------------

/// `getDecompositionLinkRow`: if `v` is a variable in the system and constraint
/// `c` does not contain `v`, we managed to decompose `v` at this point, but
/// likely not earlier when the fact involving `v` was added. In that case,
/// returns a new row for `v <= decompose(v)` to link the variable with the
/// decomposition result.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L1875>
fn get_decomposition_link_row(
    q: &Query,
    v: Side,
    c: &ConstraintTy,
    info: &mut ConstraintInfo,
) -> Row {
    let Some(&id) = info.value2index(c.is_signed).get(&v) else { return Row::new() };
    if c.coefficients.iter().any(|e| u32::from(e.id) == id) {
        return Row::new();
    }
    let mut new_variables = Vec::new();
    let decomposed = info.decompose(q, v, c.is_signed);
    let row = get_row_for_less_equal(
        &Decomposition::variable(v),
        &decomposed,
        info.value2index(c.is_signed),
        &mut new_variables,
    );
    if new_variables.is_empty() { row } else { Row::new() }
}

/// `checkCondition`: whether the systems decide `a pred b`, true or false.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L1892>
pub fn check_condition(
    q: &Query,
    pred: IntPredicate,
    a: Side,
    b: Side,
    info: &mut ConstraintInfo,
) -> Option<bool> {
    let try_with_constraint = |info: &ConstraintInfo, r: &ConstraintTy| -> Option<bool> {
        if r.is_empty() {
            return None;
        }
        r.is_implied_by(info.cs(r.is_signed))
    };

    // Retry the query after adding additional facts for A == decompose(A) and
    // B == decompose(B), if needed.
    let try_with_linked_decomposition = |info: &mut ConstraintInfo, c: &ConstraintTy| -> Option<bool> {
        if c.is_empty() {
            return None;
        }
        let num_vars = info.value2index(c.is_signed).len();
        let mut num_pushed = 0;
        for v in [a, b] {
            let row = get_decomposition_link_row(q, v, c, info);
            let negated = ConstraintSystem::<Side>::negate_or_equal(row.clone());
            if row.is_empty() || negated.is_empty() {
                continue;
            }
            let cs = info.cs_mut(c.is_signed);
            num_pushed += usize::from(cs.add_row(&row, num_vars));
            num_pushed += usize::from(cs.add_row(&negated, num_vars));
        }
        if num_pushed == 0 {
            return None;
        }
        let res = try_with_constraint(info, c);
        for _ in 0..num_pushed {
            info.cs_mut(c.is_signed).pop_last_constraint();
        }
        res
    };

    let r = info.get_constraint_for_solving(q, pred, a, b);
    if let Some(implied) = try_with_constraint(info, &r) {
        return Some(implied);
    }
    if let Some(implied) = try_with_linked_decomposition(info, &r) {
        return Some(implied);
    }

    // For non-negative operands unsigned queries can also be checked against
    // the signed system.
    if is_unsigned(pred) && is_integer(q, a) {
        let mut new_variables = Vec::new();
        let sr = info.get_constraint(q, signed_predicate(pred), a, b, &mut new_variables, false);
        if new_variables.is_empty()
            && !sr.is_empty()
            && info.is_known_non_negative(q, a)
            && info.is_known_non_negative(q, b)
        {
            if let Some(implied) = try_with_constraint(info, &sr) {
                return Some(implied);
            }
        }
    }

    // Additionally, query the signed system for eq/ne predicates if we know
    // about A or B.
    if is_equality(pred) {
        let value2index = info.value2index(true);
        if !value2index.contains_key(&a) && !value2index.contains_key(&b) {
            return None;
        }
        let mut new_variables = Vec::new();
        let sr = info.get_constraint(q, pred, a, b, &mut new_variables, true);
        if new_variables.is_empty() {
            if let Some(implied) = try_with_constraint(info, &sr) {
                return Some(implied);
            }
            if let Some(implied) = try_with_linked_decomposition(info, &sr) {
                return Some(implied);
            }
        }
    }
    None
}

// ---- worklist -----------------------------------------------------------

use llrm_mir::domtree::DomTree;
use llrm_mir::module::{BlockId, Function, InstId};
use llrm_mir::valuetracking::guarantee::{
    is_guaranteed_not_to_be_poison, is_guaranteed_to_transfer_execution_to_successor,
};
use llrm_mir::valuetracking::pattern::{CmpPredicate, IntrinsicId};

/// `ConditionTy`: a condition of the form `%op0 pred %op1`.
#[derive(Clone, Copy, Debug)]
pub struct ConditionTy {
    pub pred: CmpPredicate,
    pub op0: Side,
    pub op1: Side,
}

/// `FactOrCheck::EntryTy` with its payload.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Entry2 {
    /// A condition that holds on entry to a block, given a pre-condition that
    /// must hold for the fact to be added to the system.
    ConditionFact { cond: ConditionTy, does_hold: Option<ConditionTy> },
    /// A fact that holds after the instruction executed (an assume, a min/max
    /// intrinsic).
    InstFact { inst: InstId },
    /// An instruction to simplify, or whose flags may be strengthened, at the
    /// context instruction.
    InstCheck { inst: InstId, context_inst: InstId },
    /// A use of a compare instruction to simplify.
    UseCheck { user: InstId, index: u32 },
}

/// `FactOrCheck`: a condition fact, an assume, or a use of a compare to
/// simplify, with the dominator DFS in and out numbers of its block.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L118>
#[derive(Clone, Copy, Debug)]
pub struct FactOrCheck {
    entry: Entry2,
    pub num_in: u32,
    pub num_out: u32,
}

impl FactOrCheck {
    fn is_condition_fact(&self) -> bool {
        matches!(self.entry, Entry2::ConditionFact { .. })
    }
}

/// `getContextInstForUse`: the instruction a use is at; for a phi, the
/// terminator of the incoming block.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L72>
fn context_inst_for_use(
    function: &Function,
    user: InstId,
    index: u32,
) -> InstId {
    let instruction = function.instruction(user);
    if instruction.opcode == Opcode::Phi
        && let Some(Operand::Block(from)) = instruction.operands.get(index as usize + 1)
    {
        return function.terminator(*from).expect("a terminated incoming block");
    }
    user
}

/// `DominatorTree::findNearestCommonDominator(const Instruction *, const
/// Instruction *)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Dominators.cpp#L371>
fn nearest_common_dominator_inst(
    q: &Query,
    tree: &DomTree,
    i1: InstId,
    i2: InstId,
) -> InstId {
    let (bb1, bb2) = (q.function.parent(i1).expect("placed"), q.function.parent(i2).expect("placed"));
    if bb1 == bb2 {
        return if q.comes_before(i1, i2) { i1 } else { i2 };
    }
    if !tree.contains(bb2) {
        return i1;
    }
    if !tree.contains(bb1) {
        return i2;
    }
    let dom_bb = tree.find_nearest_common_dominator(q.function, bb1, bb2).expect("a forward tree has a block");
    if bb1 == dom_bb {
        return i1;
    }
    if bb2 == dom_bb {
        return i2;
    }
    q.function.terminator(dom_bb).expect("a terminated block")
}

/// `findCommonDominatorOfUses`: the closest program point dominating all uses
/// of `inst`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L80>
fn find_common_dominator_of_uses(
    q: &Query,
    tree: &DomTree,
    inst: InstId,
) -> InstId {
    let Some(result) = q.function.instruction(inst).result else { return inst };
    let mut common_dom: Option<InstId> = None;
    let mut num_uses = 0;
    for used in q.function.users(result) {
        // Conservatively use original instruction, if there are too many uses.
        num_uses += 1;
        if num_uses == 16 {
            return inst;
        }
        let user_i = context_inst_for_use(q.function, used.user, used.index);
        common_dom = Some(match common_dom {
            Some(dom) => nearest_common_dominator_inst(q, tree, dom, user_i),
            None => user_i,
        });
    }
    let Some(common_dom) = common_dom else { return inst };
    // Uses in unreachable blocks are not in the dominator tree.
    if tree.contains(q.function.parent(common_dom).expect("placed")) { common_dom } else { inst }
}

/// `canStrengthenFlags`: whether `inst` is a candidate whose poison-generating
/// flags may be strengthened using the constraint systems.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L1457>
fn can_strengthen_flags(
    q: &Query,
    inst: InstId,
) -> bool {
    let instruction = q.function.instruction(inst);
    let integer = instruction.result.is_some_and(|result| is_integer(q, Side::Value(Operand::Value(result))));
    if let Opcode::Cast(CastOp::Trunc) = instruction.opcode {
        return integer && instruction.flags.contains(Flags::NSW) && !instruction.flags.contains(Flags::NUW);
    }
    let Opcode::Binary(opcode) = instruction.opcode else { return false };
    if !integer {
        return false;
    }
    let both = instruction.flags.contains(Flags::NUW) && instruction.flags.contains(Flags::NSW);
    match opcode {
        BinaryOp::Sub => {
            if both {
                return false;
            }
            // A - B does not wrap unsigned if A >=u B, and does not wrap signed
            // if 0 <=s B <=s A. With a constant B, bounds on A can refine both
            // flags.
            true
        }
        BinaryOp::Add | BinaryOp::Mul | BinaryOp::Shl => {
            if both {
                return false;
            }
            // With a constant second operand, we can use bounds on the first
            // operand to refine no-wrap flags. Independently, nuw can be added
            // for nsw if the operands are non-negative.
            const_int(Side::of(q, instruction.operands[1])).is_some() || instruction.flags.contains(Flags::NSW)
        }
        _ => false,
    }
}

/// The dominator DFS numbers of `block`: `(NumIn, NumOut)`, both from
/// [`DomTree::span`] (`getDFSNumOut() - 1` for the second: only compared
/// against one another).
fn dfs_numbers(
    tree: &DomTree,
    block: BlockId,
) -> Option<(u32, u32)> {
    tree.span(block)
}

/// `State`: the state required to build the worklist.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L217>
pub struct State<'a> {
    pub tree: &'a DomTree,
    pub work_list: Vec<FactOrCheck>,
}

impl<'a> State<'a> {
    pub fn new(tree: &'a DomTree) -> Self {
        Self { tree, work_list: Vec::new() }
    }

    /// `canAddSuccessor`: whether we can add a known condition from `bb` to
    /// its successor block `succ`.
    fn can_add_successor(
        &self,
        q: &Query,
        bb: BlockId,
        succ: BlockId,
    ) -> bool {
        self.tree.dominates_edge(q.function, bb, succ, succ)
    }

    fn numbers(
        &self,
        block: BlockId,
    ) -> Option<(u32, u32)> {
        dfs_numbers(self.tree, block)
    }

    pub(crate) fn push(
        &mut self,
        block: BlockId,
        entry: Entry2,
    ) {
        let (num_in, num_out) = self.numbers(block).expect("a block of the tree");
        self.work_list.push(FactOrCheck { entry, num_in, num_out });
    }

    /// `addInfoFor`: processes block `bb` and adds known facts to the
    /// work-list.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L1506>
    pub fn add_info_for(
        &mut self,
        q: &Query,
        ind: &mut Inductions,
        bb: BlockId,
    ) {
        self.add_bounds_for_header_inductions(q, ind, bb);
        self.add_info_for_inductions(q, ind, bb);

        // True as long as the current instruction is guaranteed to execute.
        let mut guaranteed_to_execute = true;
        // Queue conditions and assumes.
        for inst in q.function.iter_block(bb) {
            let instruction = q.function.instruction(inst);
            if let Some(result) = instruction.result
                && pattern::icmp_like(q, Operand::Value(result)).is_some()
            {
                for used in q.function.users(result) {
                    let user_i = context_inst_for_use(q.function, used.user, used.index);
                    let user_block = q.function.parent(user_i).expect("placed");
                    if !self.tree.contains(user_block) {
                        continue;
                    }
                    self.push(user_block, Entry2::UseCheck { user: used.user, index: used.index });
                }
                continue;
            }

            // Facts from memory accesses.
            let access = match &instruction.opcode {
                Opcode::Load { volatile: false, .. } => Some((instruction.operands[0], instruction.ty)),
                Opcode::Store { volatile: false, .. } => {
                    q.type_of(instruction.operands[0]).map(|ty| (instruction.operands[1], ty))
                }
                _ => None,
            };
            if let Some((ptr, access_ty)) = access
                && let Some((_, gep)) = q.instruction_of(ptr)
                && matches!(gep.opcode, Opcode::GetElementPtr { .. })
            {
                let access_size = q.layout.store_size(&q.context().types, access_ty);
                if guaranteed_to_execute {
                    if let Some((pred, a, b)) = get_constraint_from_memory_access(q, ptr, access_size) {
                        // The memory access is guaranteed to execute when BB
                        // is entered, hence the constraint holds on entry to
                        // BB.
                        self.push(
                            bb,
                            Entry2::ConditionFact { cond: ConditionTy { pred, op0: a, op1: b }, does_hold: None },
                        );
                    }
                } else {
                    self.push(bb, Entry2::InstFact { inst });
                }
            }

            let id = pattern::call_intrinsic(q, inst);
            match id {
                Some(IntrinsicId::Assume) => {
                    let Some((pred, a, b)) = pattern::icmp_like(q, instruction.operands[0]) else {
                        continue_after(&mut guaranteed_to_execute, q, inst);
                        continue;
                    };
                    let cond = ConditionTy { pred, op0: a, op1: b };
                    if guaranteed_to_execute {
                        // The assume is guaranteed to execute when BB is
                        // entered, hence Cond holds on entry to BB.
                        self.push(bb, Entry2::ConditionFact { cond, does_hold: None });
                    } else {
                        self.push(bb, Entry2::InstFact { inst });
                    }
                }
                // Enqueue intrinsics for simplification.
                Some(
                    IntrinsicId::UaddWithOverflow
                    | IntrinsicId::SaddWithOverflow
                    | IntrinsicId::UsubWithOverflow
                    | IntrinsicId::SsubWithOverflow
                    | IntrinsicId::UmulWithOverflow
                    | IntrinsicId::SmulWithOverflow
                    | IntrinsicId::Ucmp
                    | IntrinsicId::Scmp,
                ) => self.push(bb, Entry2::InstCheck { inst, context_inst: inst }),
                // Enqueue the intrinsics to add extra info.
                Some(
                    id @ (IntrinsicId::Umin
                    | IntrinsicId::Umax
                    | IntrinsicId::Smin
                    | IntrinsicId::Smax
                    | IntrinsicId::UsubSat
                    | IntrinsicId::UaddSat),
                ) => {
                    if id != IntrinsicId::UaddSat {
                        self.push(bb, Entry2::InstCheck { inst, context_inst: inst });
                    }
                    // TODO in LLVM: Check if it is possible to instead only
                    // added the min/max facts when simplifying uses of the
                    // min/max intrinsics.
                    if let Some(result) = instruction.result
                        && is_guaranteed_not_to_be_poison(q, Operand::Value(result), 0)
                    {
                        self.push(bb, Entry2::InstFact { inst });
                    }
                }
                Some(IntrinsicId::Abs) => self.push(bb, Entry2::InstFact { inst }),
                _ => {}
            }

            // Add facts from unsigned division, remainder and logical shift
            // right, and from signed division and remainder.
            //   urem x, n: result < n  and  result <= x
            //   udiv x, n: result <= x
            //   lshr x, n: result <= x
            //   srem x, n: result >= 0 and result <= x, if x >= 0
            //              result < n,                  if n > 0
            //   sdiv x, n: result >= 0 and result <= x, if x >= 0 and n > 0
            //              result >= 0 and result < x,  if x > 0 and n > 1
            if let Opcode::Binary(opcode) = instruction.opcode
                && matches!(
                    opcode,
                    BinaryOp::URem | BinaryOp::UDiv | BinaryOp::LShr | BinaryOp::SRem | BinaryOp::SDiv
                )
                && let Some(result) = instruction.result
                && is_guaranteed_not_to_be_poison(q, Operand::Value(result), 0)
            {
                self.push(bb, Entry2::InstFact { inst });
            }

            // Queue instructions whose flags may be strengthened, checked at
            // the closest point dominating all uses.
            if can_strengthen_flags(q, inst) {
                let common_dom = find_common_dominator_of_uses(q, self.tree, inst);
                let block = q.function.parent(common_dom).expect("placed");
                self.push(block, Entry2::InstCheck { inst, context_inst: common_dom });
            }

            guaranteed_to_execute &= is_guaranteed_to_transfer_execution_to_successor(q, inst);
        }

        let Some(terminator) = q.function.terminator(bb) else { return };
        let terminator_inst = q.function.instruction(terminator);
        if terminator_inst.opcode == Opcode::Switch {
            // operands: condition, default, then (value, block) pairs.
            let cond = Side::of(q, terminator_inst.operands[0]);
            for pair in terminator_inst.operands[2..].chunks(2) {
                let (Operand::Block(succ), value) = (pair[1], pair[0]) else { continue };
                if !self.can_add_successor(q, bb, succ) {
                    continue;
                }
                let cond =
                    ConditionTy { pred: CmpPredicate::new(IntPredicate::Eq), op0: cond, op1: Side::of(q, value) };
                self.push(succ, Entry2::ConditionFact { cond, does_hold: None });
            }
            return;
        }

        // A conditional branch.
        if terminator_inst.opcode != Opcode::Br || terminator_inst.operands.len() != 3 {
            return;
        }
        let cond = terminator_inst.operands[0];
        let (Operand::Block(succ0), Operand::Block(succ1)) = (terminator_inst.operands[1], terminator_inst.operands[2])
        else {
            return;
        };

        // If the condition is a chain of ORs/AND and the successor only has
        // the current block as predecessor, queue conditions for the
        // successor.
        let or = pattern::logical_or(q, cond);
        let and = pattern::logical_and(q, cond);
        if let Some(operands) = or.or(and) {
            let is_or = or.is_some();
            // If there's a select that matches both AND and OR, we need to
            // commit to one of the options. Arbitrarily pick OR.
            let (op0, op1) = operands;
            let successor = if is_or { succ1 } else { succ0 };
            if self.can_add_successor(q, bb, successor) {
                let mut cond_work_list: Vec<Operand> = Vec::new();
                let mut seen: Vec<Operand> = Vec::new();
                let mut queue = |v: Operand, list: &mut Vec<Operand>| {
                    if !seen.contains(&v) {
                        seen.push(v);
                        list.push(v);
                    }
                };
                queue(op1, &mut cond_work_list);
                queue(op0, &mut cond_work_list);
                while let Some(cur) = cond_work_list.pop() {
                    if let Some((pred, a, b)) = pattern::icmp_like(q, cur) {
                        let pred = if is_or { pred.inverse() } else { pred };
                        let cond = ConditionTy { pred, op0: a, op1: b };
                        self.push(successor, Entry2::ConditionFact { cond, does_hold: None });
                        continue;
                    }
                    let inner = if is_or { pattern::logical_or(q, cur) } else { pattern::logical_and(q, cur) };
                    if let Some((op0, op1)) = inner {
                        queue(op1, &mut cond_work_list);
                        queue(op0, &mut cond_work_list);
                    }
                }
            }
            return;
        }

        let Some((pred, a, b)) = pattern::icmp_like(q, cond) else { return };
        if self.can_add_successor(q, bb, succ0) {
            let cond = ConditionTy { pred, op0: a, op1: b };
            self.push(succ0, Entry2::ConditionFact { cond, does_hold: None });
        }
        if self.can_add_successor(q, bb, succ1) {
            let cond = ConditionTy { pred: pred.inverse(), op0: a, op1: b };
            self.push(succ1, Entry2::ConditionFact { cond, does_hold: None });
        }
    }
}

fn continue_after(
    guaranteed_to_execute: &mut bool,
    q: &Query,
    inst: InstId,
) {
    *guaranteed_to_execute &= is_guaranteed_to_transfer_execution_to_successor(q, inst);
}

// ---- the pass -----------------------------------------------------------

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{GlobalValue, MetadataNode};

/// `MaxRows`, `-constraint-elimination-max-rows`.
const MAX_ROWS: usize = 500;

/// What a check found, to be done once the query that found it is dropped:
/// the function and the context are edited, not read.
enum Action {
    /// `ReplaceCmpWithConstant`: the compare `check` is `is_true` where the
    /// facts of the block hold.
    ReplaceCmp { check: InstId, is_true: bool, uses: Vec<llrm_mir::module::Use> },
    /// `ReplaceMinMaxWithOperand`: the min/max `inst` is its operand `use_lhs`.
    ReplaceWith { inst: InstId, with: Operand },
    /// `checkAndReplaceCmp`: the three-way compare is the constant `value`.
    ReplaceWithInt { inst: InstId, value: i128 },
    /// `checkAndReplaceUSubSat`: `usub.sat(a, b)` is `sub nuw a, b`.
    USubSat { inst: InstId, nsw: bool },
    /// `tryToSimplifyOverflowMath`.
    Overflow { inst: InstId, op: BinaryOp, signed: bool },
    /// `tryToStrengthenFlags`: the flags `inst` gains.
    Flags { inst: InstId, flags: Flags },
    /// `checkOrAndOpImpliedByOther`: the join `inst` is `with`, or `with` a
    /// constant of its type.
    Join { inst: InstId, with: JoinWith },
}

enum JoinWith {
    Operand(Operand),
    Bool(bool),
}

/// The sort of `eliminateConstraints`: dominating conditions to check and
/// facts come before conditions and facts dominated by them. If a condition to
/// check and a fact have the same numbers, conditional facts come first.
/// Assume facts and checks are ordered according to their relative order in the
/// containing basic block. Also make sure conditions with constant operands
/// come before conditions without constant operands. This increases the
/// effectiveness of the current signed <-> unsigned fact transfer logic.
fn fact_before(
    q: &Query,
    positions: &[u32],
    a: &FactOrCheck,
    b: &FactOrCheck,
) -> bool {
    let has_no_const_op = |c: &FactOrCheck| {
        let Entry2::ConditionFact { cond, .. } = c.entry else { unreachable!("only condition facts are compared") };
        const_int(cond.op0).is_none() && const_int(cond.op1).is_none()
    };
    // If both entries have the same In numbers, conditional facts come first.
    // Otherwise use the relative order in the basic block.
    if a.num_in == b.num_in {
        if a.is_condition_fact() && b.is_condition_fact() {
            return !has_no_const_op(a) && has_no_const_op(b);
        }
        if a.is_condition_fact() {
            return true;
        }
        if b.is_condition_fact() {
            return false;
        }
        let (inst_a, inst_b) = (context_inst_of(q, a), context_inst_of(q, b));
        let (block_a, block_b) = (q.function.parent(inst_a), q.function.parent(inst_b));
        return block_a == block_b
            && positions.get(inst_a.0 as usize).copied().unwrap_or(0)
                < positions.get(inst_b.0 as usize).copied().unwrap_or(0);
    }
    a.num_in < b.num_in
}

/// `FactOrCheck::getContextInst`.
fn context_inst_of(
    q: &Query,
    c: &FactOrCheck,
) -> InstId {
    match c.entry {
        Entry2::UseCheck { user, index } => context_inst_for_use(q.function, user, index),
        Entry2::InstCheck { context_inst, .. } => context_inst,
        Entry2::InstFact { inst } => inst,
        Entry2::ConditionFact { .. } => unreachable!("a condition fact has no context instruction"),
    }
}

/// `eliminateConstraints`: whether the function changed.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L2361>
pub fn eliminate_constraints(
    context: &Context,
    layout: &DataLayout,
    declarations: &[GlobalValue],
    metadata: &[MetadataNode],
    function: &mut Function,
) -> bool {
    let mut changed = false;
    if function.entry().is_none() {
        return false;
    }
    let mut tree = DomTree::new();
    tree.recalculate(&*function);
    tree.update_dfs_numbers();
    let function_args: Vec<Side> =
        function.parameters().iter().map(|&value| Side::Value(Operand::Value(value))).collect();
    let mut info = ConstraintInfo::new(&function_args);
    let positions = function.positions();
    let mut to_remove: Vec<InstId> = Vec::new();
    let mut dfs_in_stack: Vec<StackEntry> = Vec::new();

    let work_list = {
        let mut q = Query::bare(&*context, &*function, layout);
        q.globals = declarations;
        q.metadata = metadata;
        let mut state = State::new(&tree);
        let mut inductions = Inductions::new(&q, &tree);
        // First, collect conditions implied by branches and blocks with their
        // Dominator DFS in and out numbers.
        for &bb in function.layout() {
            if !tree.contains(bb) {
                continue;
            }
            state.add_info_for(&q, &mut inductions, bb);
        }
        // Next, sort worklist by dominance (see `fact_before`).
        let mut work_list = std::mem::take(&mut state.work_list);
        work_list.sort_by(|a, b| {
            if fact_before(&q, &positions, a, b) {
                std::cmp::Ordering::Less
            } else if fact_before(&q, &positions, b, a) {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
        work_list
    };

    // Finally, process ordered worklist and eliminate implied conditions.
    for cb in &work_list {
        // First, pop entries from the stack that are out-of-scope for CB.
        // Remove the corresponding entry from the constraint system.
        while let Some(top) = dfs_in_stack.last() {
            debug_assert!(top.num_in <= cb.num_in);
            if cb.num_out <= top.num_out {
                break;
            }
            let entry = top.clone();
            info.remove_entry_from_stack(&entry, &mut dfs_in_stack);
        }

        let mut actions: Vec<Action> = Vec::new();
        {
            let mut q = Query::bare(&*context, &*function, layout);
            q.globals = declarations;
            q.metadata = metadata;
            process(&q, &tree, &positions, &mut info, cb, &mut dfs_in_stack, &mut actions);
        }
        for action in actions {
            changed |= apply(context, function, &tree, &positions, action, &mut to_remove);
        }
    }

    debug_assert_eq!(
        info.cs(false).size() - function_args.len(),
        dfs_in_stack.iter().filter(|e| !e.is_signed).count(),
        "updates to CS and DFSInStack are out of sync"
    );
    debug_assert_eq!(
        info.cs(true).size(),
        dfs_in_stack.iter().filter(|e| e.is_signed).count(),
        "updates to CS and DFSInStack are out of sync"
    );

    for inst in to_remove {
        function.erase(inst).expect("a removed instruction has no users left");
    }
    changed
}

/// The result of `inst` as a side.
fn result_side(
    q: &Query,
    inst: InstId,
) -> Option<Side> {
    q.function.instruction(inst).result.map(|result| Side::of(q, Operand::Value(result)))
}

/// The uses of the compare `check` that `ReplaceCmpWithConstant` replaces: in
/// blocks the fact's DFS range holds in, not before `context_inst` in its
/// block, and not an assume's.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L1993>
fn qualifying_uses(
    q: &Query,
    tree: &DomTree,
    positions: &[u32],
    check: InstId,
    context_inst: InstId,
    num_in: u32,
    num_out: u32,
) -> Vec<llrm_mir::module::Use> {
    let Some(result) = q.function.instruction(check).result else { return Vec::new() };
    let position = |inst: InstId| positions.get(inst.0 as usize).copied().unwrap_or(u32::MAX);
    q.function
        .users(result)
        .iter()
        .filter(|used| {
            let user_i = context_inst_for_use(q.function, used.user, used.index);
            let user_block = q.function.parent(user_i).expect("placed");
            let Some((dtn_in, dtn_out)) = dfs_numbers(tree, user_block) else { return false };
            if dtn_in < num_in || dtn_out > num_out {
                return false;
            }
            if q.function.parent(context_inst) == Some(user_block) && position(user_i) < position(context_inst) {
                return false;
            }
            // Conditions in an assume trivially simplify to true. Skip uses in
            // assume calls to not destroy the available information.
            pattern::call_intrinsic(q, used.user) != Some(IntrinsicId::Assume)
        })
        .copied()
        .collect()
}

/// The checks and facts of one worklist entry, against the systems as they
/// stand.
fn process(
    q: &Query,
    tree: &DomTree,
    positions: &[u32],
    info: &mut ConstraintInfo,
    cb: &FactOrCheck,
    dfs_in_stack: &mut Vec<StackEntry>,
    actions: &mut Vec<Action>,
) {
    match cb.entry {
        Entry2::InstCheck { .. } | Entry2::UseCheck { .. } => {
            process_check(q, tree, positions, info, cb, dfs_in_stack, actions)
        }
        Entry2::InstFact { inst } => process_inst_fact(q, info, cb, dfs_in_stack, inst),
        Entry2::ConditionFact { cond, does_hold } => {
            if let Some(pre) = does_hold
                && !info.does_hold(q, pre.pred.pred, pre.op0, pre.op1)
            {
                return;
            }
            add_fact(q, info, cb, dfs_in_stack, cond.pred, cond.op0, cond.op1);
        }
    }
}

/// The `AddFact` lambda of `eliminateConstraints`.
fn add_fact(
    q: &Query,
    info: &mut ConstraintInfo,
    cb: &FactOrCheck,
    dfs_in_stack: &mut Vec<StackEntry>,
    pred: CmpPredicate,
    a: Side,
    b: Side,
) {
    if info.cs(is_signed(pred.pred)).size() > MAX_ROWS {
        return;
    }

    info.add_fact(q, pred.pred, a, b, cb.num_in, cb.num_out, dfs_in_stack);

    if !is_equality(pred.pred) {
        // If samesign is present on the ICmp, simply flip the sign of the
        // predicate, transferring the information from the signed system to
        // the unsigned system, and viceversa.
        if pred.same_sign {
            info.add_fact(q, CmpPredicate::flipped_signedness(pred.pred), a, b, cb.num_in, cb.num_out, dfs_in_stack);
        } else {
            info.transfer_to_other_system(q, pred.pred, a, b, cb.num_in, cb.num_out, dfs_in_stack);
        }
    }

    // (X | Y) >s -1 implies X >s -1 and Y >s -1, because the sign bit of an OR
    // is the OR of the operand sign bits. Similarly, (X & Y) <s 0 implies X <s
    // 0 and Y <s 0. Look through these canonical forms produced by InstCombine
    // so the sign facts on the operands are available to the solver.
    if (pred.pred == IntPredicate::Sgt && is_all_ones(q, b)) || (pred.pred == IntPredicate::Slt && is_zero(q, b)) {
        let opc = if pred.pred == IntPredicate::Sgt { BinaryOp::Or } else { BinaryOp::And };
        let mut worklist = vec![a];
        let mut seen: Vec<Side> = Vec::new();
        while let Some(cur) = worklist.pop() {
            let Side::Value(op) = cur else { continue };
            let Some((_, _, op0, op1)) = pattern::binary(q, op, opc) else { continue };
            for operand in [op0, op1] {
                let operand = Side::of(q, operand);
                if seen.contains(&operand) {
                    continue;
                }
                seen.push(operand);
                worklist.push(operand);
                info.add_fact(q, pred.pred, operand, b, cb.num_in, cb.num_out, dfs_in_stack);
            }
        }
    }
}

/// The part of `eliminateConstraints` for a fact that is not a condition: an
/// assume, a min/max, an `abs`, saturating arithmetic, a division or a
/// remainder.
fn process_inst_fact(
    q: &Query,
    info: &mut ConstraintInfo,
    cb: &FactOrCheck,
    dfs_in_stack: &mut Vec<StackEntry>,
    inst: InstId,
) {
    let instruction = q.function.instruction(inst);
    // `AddFactsAboutIndices`: a load or store not guaranteed to execute.
    let access = match &instruction.opcode {
        Opcode::Load { .. } => Some((instruction.operands[0], instruction.ty)),
        Opcode::Store { .. } => q.type_of(instruction.operands[0]).map(|ty| (instruction.operands[1], ty)),
        _ => None,
    };
    if let Some((ptr, access_ty)) = access {
        let access_size = q.layout.store_size(&q.context().types, access_ty);
        if let Some((pred, a, b)) = get_constraint_from_memory_access(q, ptr, access_size) {
            add_fact(q, info, cb, dfs_in_stack, pred, a, b);
        }
        return;
    }
    let Some(this) = result_side(q, inst) else {
        // An assume whose condition did not hold on entry (not guaranteed to
        // execute): its compare.
        let Some((pred, a, b)) = pattern::icmp_like(q, instruction.operands[0]) else { return };
        add_fact(q, info, cb, dfs_in_stack, pred, a, b);
        return;
    };
    let zero = int_like(q, this, 0);
    let mut fact = |info: &mut ConstraintInfo, pred: IntPredicate, a: Side, b: Side| {
        add_fact(q, info, cb, dfs_in_stack, CmpPredicate::new(pred), a, b);
    };
    match pattern::call_intrinsic(q, inst) {
        Some(IntrinsicId::Abs) => {
            let x = Side::of(q, instruction.operands[0]);
            // If is_int_min_poison is true then we may assume llvm.abs >= 0.
            if const_int(Side::of(q, instruction.operands[1])).is_some_and(|c| c.bits == 1) {
                fact(info, IntPredicate::Sge, this, zero);
            }
            fact(info, IntPredicate::Sge, this, x);
            return;
        }
        Some(id @ (IntrinsicId::Smax | IntrinsicId::Smin | IntrinsicId::Umax | IntrinsicId::Umin)) => {
            let pred = match id {
                IntrinsicId::Smax => IntPredicate::Sge,
                IntrinsicId::Smin => IntPredicate::Sle,
                IntrinsicId::Umax => IntPredicate::Uge,
                _ => IntPredicate::Ule,
            };
            fact(info, pred, this, Side::of(q, instruction.operands[0]));
            fact(info, pred, this, Side::of(q, instruction.operands[1]));
            return;
        }
        Some(IntrinsicId::UaddSat) => {
            fact(info, IntPredicate::Uge, this, Side::of(q, instruction.operands[0]));
            fact(info, IntPredicate::Uge, this, Side::of(q, instruction.operands[1]));
            return;
        }
        Some(IntrinsicId::UsubSat) => {
            fact(info, IntPredicate::Ule, this, Side::of(q, instruction.operands[0]));
            return;
        }
        _ => {}
    }

    let Opcode::Binary(opcode) = instruction.opcode else { return };
    let x = Side::of(q, instruction.operands[0]);
    let n = Side::of(q, instruction.operands[1]);
    match opcode {
        BinaryOp::URem => {
            // urem x, n: result < n (remainder is always less than divisor)
            fact(info, IntPredicate::Ult, this, n);
            // urem x, n: result <= x (remainder is at most the dividend)
            fact(info, IntPredicate::Ule, this, x);
        }
        // udiv x, n: result <= x (quotient is at most the dividend)
        BinaryOp::UDiv => fact(info, IntPredicate::Ule, this, x),
        // lshr x, n: result <= x (right shift cannot increase the value)
        BinaryOp::LShr => fact(info, IntPredicate::Ule, this, x),
        BinaryOp::SRem => {
            if info.does_hold(q, IntPredicate::Sge, x, zero) || side_is_known_non_negative(q, x, 0) {
                // srem x, n: result >= 0, if x >= 0 (result has the sign of x)
                fact(info, IntPredicate::Sge, this, zero);
                // srem x, n: result <= x, if x >= 0 (|result| <= |x| and both
                // are non-negative)
                fact(info, IntPredicate::Sle, this, x);
            }
            if info.does_hold(q, IntPredicate::Sge, n, zero) || side_is_known_positive(q, n) {
                // srem x, n: result <= n, if n >= 0 (|result| < n, so result
                // <= n - 1
                fact(info, IntPredicate::Slt, this, n);
            }
        }
        BinaryOp::SDiv => {
            if !info.is_known_non_negative(q, x) || !info.is_known_positive(q, n) {
                return;
            }
            let is_strict = info.is_known_positive(q, x) && info.does_hold(q, IntPredicate::Sgt, n, int_like(q, n, 1));
            fact(info, IntPredicate::Sge, this, zero);
            fact(info, if is_strict { IntPredicate::Slt } else { IntPredicate::Sle }, this, x);
        }
        _ => {}
    }
}

fn side_is_known_positive(
    q: &Query,
    s: Side,
) -> bool {
    match s {
        Side::Int(bits, width) => signed(bits, width) > 0,
        Side::Value(op) => is_known_positive(q, op, 0),
    }
}

/// The part of `eliminateConstraints` for a check.
fn process_check(
    q: &Query,
    tree: &DomTree,
    positions: &[u32],
    info: &mut ConstraintInfo,
    cb: &FactOrCheck,
    dfs_in_stack: &mut Vec<StackEntry>,
    actions: &mut Vec<Action>,
) {
    // `getInstructionToSimplify`: the use may have been simplified to a
    // constant already.
    let inst = match cb.entry {
        Entry2::InstCheck { inst, .. } => inst,
        Entry2::UseCheck { user, index } => {
            let Some(Operand::Value(value)) = q.function.instruction(user).operands.get(index as usize).copied() else {
                return;
            };
            let llrm_mir::module::ValueDef::Instruction(inst) = q.function.value(value).def else { return };
            inst
        }
        _ => unreachable!("a check"),
    };
    let context_inst = context_inst_of(q, cb);
    let Some(this) = result_side(q, inst) else { return };

    if can_strengthen_flags(q, inst) {
        let mut flags = q.function.instruction(inst).flags;
        let before = flags;
        if !flags.contains(Flags::NSW) && is_known_no_wrap(this, info, q, true) {
            flags.insert(Flags::NSW);
        }
        if !flags.contains(Flags::NUW) && is_known_no_wrap_with(this, info, q, false, Some(flags)) {
            flags.insert(Flags::NUW);
        }
        if flags != before {
            actions.push(Action::Flags { inst, flags });
        }
        return;
    }

    let instruction = q.function.instruction(inst);
    let Some(result) = instruction.result else { return };
    let id = pattern::call_intrinsic(q, inst);
    if let Some((_, _)) = pattern::with_overflow_inst(q, Operand::Value(result)) {
        if let Some(llrm_mir::intrinsics::Intrinsic::WithOverflow { op, signed }) = q.intrinsic(inst)
            && is_known_no_wrap(this, info, q, signed)
        {
            actions.push(Action::Overflow { inst, op, signed });
        }
    } else if let Some((pred, a, b)) = pattern::icmp_like(q, Operand::Value(result)) {
        let simplified =
            check_and_replace_condition(q, tree, positions, info, pred, a, b, inst, context_inst, cb, actions);
        if !simplified
            && q.function
                .instruction(context_inst)
                .result
                .is_some_and(|value| pattern::logical_op(q, Operand::Value(value)).is_some())
        {
            check_or_and_op_implied_by_other(q, info, cb, context_inst, inst, dfs_in_stack, actions);
        }
    } else if matches!(
        id,
        Some(IntrinsicId::Smax | IntrinsicId::Smin | IntrinsicId::Umax | IntrinsicId::Umin)
    ) {
        let pred = match id {
            Some(IntrinsicId::Smax) => IntPredicate::Sge,
            Some(IntrinsicId::Smin) => IntPredicate::Sle,
            Some(IntrinsicId::Umax) => IntPredicate::Uge,
            _ => IntPredicate::Ule,
        };
        let (op0, op1) = (Side::of(q, instruction.operands[0]), Side::of(q, instruction.operands[1]));
        if let Some(implied) = check_condition(q, pred, op0, op1, info) {
            actions.push(Action::ReplaceWith {
                inst,
                with: if implied { instruction.operands[0] } else { instruction.operands[1] },
            });
        } else if let Some(implied) = check_condition(q, pred, op1, op0, info) {
            actions.push(Action::ReplaceWith {
                inst,
                with: if !implied { instruction.operands[0] } else { instruction.operands[1] },
            });
        }
    } else if matches!(id, Some(IntrinsicId::Ucmp | IntrinsicId::Scmp)) {
        let (lhs, rhs) = (Side::of(q, instruction.operands[0]), Side::of(q, instruction.operands[1]));
        let (gt, lt) = if id == Some(IntrinsicId::Scmp) {
            (IntPredicate::Sgt, IntPredicate::Slt)
        } else {
            (IntPredicate::Ugt, IntPredicate::Ult)
        };
        if check_condition(q, gt, lhs, rhs, info).unwrap_or(false) {
            actions.push(Action::ReplaceWithInt { inst, value: 1 });
        } else if check_condition(q, lt, lhs, rhs, info).unwrap_or(false) {
            actions.push(Action::ReplaceWithInt { inst, value: -1 });
        } else if check_condition(q, IntPredicate::Eq, lhs, rhs, info).unwrap_or(false) {
            actions.push(Action::ReplaceWithInt { inst, value: 0 });
        }
    } else if id == Some(IntrinsicId::UsubSat) {
        // usub.sat(A, B) is A - B exactly when A >=u B.
        let (a, b) = (Side::of(q, instruction.operands[0]), Side::of(q, instruction.operands[1]));
        if check_condition(q, IntPredicate::Uge, a, b, info).unwrap_or(false) {
            let nsw = info.is_known_non_negative(q, a);
            actions.push(Action::USubSat { inst, nsw });
        }
    }
}

/// `checkAndReplaceCondition`: whether any use of the compare `check` is
/// replaced.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L1987>
#[allow(clippy::too_many_arguments)]
fn check_and_replace_condition(
    q: &Query,
    tree: &DomTree,
    positions: &[u32],
    info: &mut ConstraintInfo,
    pred: CmpPredicate,
    a: Side,
    b: Side,
    check: InstId,
    context_inst: InstId,
    cb: &FactOrCheck,
    actions: &mut Vec<Action>,
) -> bool {
    let replace = |is_true: bool, actions: &mut Vec<Action>| {
        let uses = qualifying_uses(q, tree, &positions, check, context_inst, cb.num_in, cb.num_out);
        let changed = !uses.is_empty();
        actions.push(Action::ReplaceCmp { check, is_true, uses });
        changed
    };
    if let Some(implied) = check_condition(q, pred.pred, a, b, info) {
        return replace(implied, actions);
    }
    // When the predicate is samesign and unsigned, we can also make use of the
    // signed predicate information.
    if pred.same_sign
        && is_unsigned(pred.pred)
        && let Some(implied) = check_condition(q, signed_predicate(pred.pred), a, b, info)
    {
        return replace(implied, actions);
    }
    false
}

/// `checkOrAndOpImpliedByOther`: whether either the first condition of an AND
/// or OR is implied by the (negated in case of OR) second condition or vice
/// versa.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L2131>
fn check_or_and_op_implied_by_other(
    q: &Query,
    info: &mut ConstraintInfo,
    cb: &FactOrCheck,
    join_op: InstId,
    cmp_to_check: InstId,
    dfs_in_stack: &mut Vec<StackEntry>,
    actions: &mut Vec<Action>,
) -> bool {
    let join = q.function.instruction(join_op);
    let Some(join_result) = join.result else { return false };
    if q.function.users(join_result).is_empty() {
        return false;
    }
    let cmp_result = q.function.instruction(cmp_to_check).result.expect("a compare has a result");
    let other_op_idx = usize::from(join.operands[0] == Operand::Value(cmp_result));

    // Don't try to simplify the first condition of a select by the second, as
    // this may make the select more poisonous than the original one.
    // TODO in LLVM: check if the first operand may be poison.
    if other_op_idx != 0 && join.opcode == Opcode::Select {
        return false;
    }

    let old_size = dfs_in_stack.len();
    let is_or = pattern::logical_or(q, Operand::Value(join_result)).is_some();
    let mut worklist = vec![join.operands[other_op_idx]];
    // Do a traversal of the AND/OR tree to add facts from leaf compares.
    while let Some(val) = worklist.pop() {
        if let Some((pred, lhs, rhs)) = pattern::icmp_like(q, val) {
            // For OR, check if the negated condition implies CmpToCheck.
            let pred = if is_or { pred.inverse() } else { pred };
            // Optimistically add fact from the other compares in the AND/OR.
            info.add_fact(q, pred.pred, lhs, rhs, cb.num_in, cb.num_out, dfs_in_stack);
            continue;
        }
        let inner = if is_or { pattern::logical_or(q, val) } else { pattern::logical_and(q, val) };
        if let Some((lhs, rhs)) = inner {
            worklist.push(lhs);
            worklist.push(rhs);
        }
    }
    let mut result = false;
    if old_size != dfs_in_stack.len() {
        let (pred, a, b) = pattern::icmp_like(q, Operand::Value(cmp_result)).expect("expected icmp-like match");
        // Check if the second condition can be simplified now.
        if let Some(implied) = check_condition(q, pred.pred, a, b, info) {
            let with =
                if is_or == implied { JoinWith::Bool(implied) } else { JoinWith::Operand(join.operands[other_op_idx]) };
            actions.push(Action::Join { inst: join_op, with });
            result = true;
        }
    }
    // Remove entries again.
    while old_size < dfs_in_stack.len() {
        let entry = dfs_in_stack.last().cloned().expect("an entry");
        info.remove_entry_from_stack(&entry, dfs_in_stack);
    }
    result
}

/// Does what a check found.
fn apply(
    context: &Context,
    function: &mut Function,
    tree: &DomTree,
    positions: &[u32],
    action: Action,
    to_remove: &mut Vec<InstId>,
) -> bool {
    let _ = (tree, positions);
    match action {
        Action::ReplaceCmp { check, is_true, uses } => {
            let ty = function.instruction(check).ty;
            let constant = Operand::Constant(context.int(ty, i128::from(is_true)));
            let changed = !uses.is_empty();
            for used in uses {
                function.set_operand(used.user, used.index as usize, constant);
            }
            if let Some(result) = function.instruction(check).result
                && function.users(result).is_empty()
            {
                to_remove.push(check);
            }
            changed
        }
        Action::ReplaceWith { inst, with } => {
            let result = function.instruction(inst).result.expect("a min/max has a result");
            function.replace_all_uses_with(result, with);
            to_remove.push(inst);
            true
        }
        Action::ReplaceWithInt { inst, value } => {
            let ty = function.instruction(inst).ty;
            let constant = Operand::Constant(context.int(ty, value));
            let result = function.instruction(inst).result.expect("a compare has a result");
            function.replace_all_uses_with(result, constant);
            to_remove.push(inst);
            true
        }
        Action::USubSat { inst, nsw } => {
            let instruction = function.instruction(inst);
            let (ty, a, b) = (instruction.ty, instruction.operands[0], instruction.operands[1]);
            let mut flags = Flags::NUW;
            if nsw {
                flags.insert(Flags::NSW);
            }
            let sub = function.create_instruction(Opcode::Binary(BinaryOp::Sub), ty, vec![a, b], flags, None);
            function.insert(sub, Position::Before(inst)).expect("a placed instruction");
            let new = Operand::Value(function.instruction(sub).result.expect("a sub has a result"));
            let old = function.instruction(inst).result.expect("usub.sat has a result");
            function.replace_all_uses_with(old, new);
            to_remove.push(inst);
            true
        }
        Action::Overflow { inst, op, signed } => replace_overflow_uses(context, function, inst, op, signed, to_remove),
        Action::Flags { inst, flags } => {
            function.set_flags(inst, flags);
            true
        }
        Action::Join { inst, with } => {
            let result = function.instruction(inst).result.expect("a logical op has a result");
            let with = match with {
                JoinWith::Operand(operand) => operand,
                JoinWith::Bool(value) => {
                    let ty = function.instruction(inst).ty;
                    Operand::Constant(context.int(ty, i128::from(value)))
                }
            };
            function.replace_all_uses_with(result, with);
            to_remove.push(inst);
            true
        }
    }
}

/// `replaceOverflowUses`: replaces the uses of `inst`, which is known not to
/// overflow, by the corresponding plain binary operation and a false overflow
/// flag.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L2315>
fn replace_overflow_uses(
    context: &Context,
    function: &mut Function,
    inst: InstId,
    op: BinaryOp,
    is_signed_op: bool,
    to_remove: &mut Vec<InstId>,
) -> bool {
    let mut changed = false;
    let instruction = function.instruction(inst);
    let args: Vec<Operand> = instruction.operands[..instruction.operands.len() - 1].to_vec();
    let aggregate = instruction.result.expect("a with.overflow call has a result");
    let value_ty = function.operand_type(context, args[0]).expect("an operand has a type");
    let mut res: Option<Operand> = None;
    for used in function.users(aggregate).snapshot() {
        let user = function.instruction(used.user);
        let Opcode::ExtractValue(indices) = &user.opcode else { continue };
        let Some(user_result) = user.result else { continue };
        let replacement = match indices[..] {
            [0] => {
                let operand = match res {
                    Some(operand) => operand,
                    None => {
                        let flags = if is_signed_op { Flags::NSW } else { Flags::NUW };
                        let binary = function.create_instruction(
                            Opcode::Binary(op),
                            value_ty,
                            vec![args[0], args[1]],
                            flags,
                            None,
                        );
                        function.insert(binary, Position::Before(inst)).expect("a placed instruction");
                        let operand = Operand::Value(function.instruction(binary).result.expect("a result"));
                        res = Some(operand);
                        operand
                    }
                };
                operand
            }
            [1] => {
                let i1 = function.instruction(used.user).ty;
                Operand::Constant(context.int(i1, 0))
            }
            _ => continue,
        };
        function.replace_all_uses_with(user_result, replacement);
        changed = true;
        if function.users(user_result).is_empty() {
            let poison = Operand::Constant(
                context.constant(Constant { ty: function.instruction(inst).ty, kind: ConstantKind::Poison }),
            );
            function.set_operand(used.user, 0, poison);
            to_remove.push(used.user);
            changed = true;
        }
    }

    if function.users(aggregate).is_empty() {
        // Do not erase II here: the worklist may still hold Uses of II's
        // operands.
        for (index, arg) in args.iter().enumerate() {
            let ty = function.operand_type(context, *arg).expect("an operand has a type");
            let poison = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Poison }));
            function.set_operand(inst, index, poison);
        }
        to_remove.push(inst);
        changed = true;
    }
    changed
}

// ---- the function pass --------------------------------------------------

use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};

/// `ConstraintEliminationPass`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp#L2697>
pub struct ConstraintElimination;

impl FunctionPass for ConstraintElimination {
    fn name(&self) -> &'static str {
        "constraint-elimination"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let declarations: &[GlobalValue] = outer.cached_ref::<passes::Declarations>().map_or(&[], Vec::as_slice);
        if eliminate_constraints(unit.context, unit.layout, declarations, unit.metadata, unit.function) {
            // The CFG is as it was.
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}
