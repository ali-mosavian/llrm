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
//! This file is the engine: [`Decomposition`], [`ConstraintTy`] and
//! [`ConstraintInfo`] (`decompose`, `getConstraint`, `isImpliedBy`, `addFact`,
//! `transferToOtherSystem`, `checkCondition`).
//!
//! DEVIATE (anything MIR lacks):
//! 1. A variable is an `Operand`: a value or a constant (a global, `null`),
//!    where LLVM's is a `Value *`.
//! 2. A constant-expression GEP is not a `GEPOperator` here: only a `gep`
//!    instruction is decomposed (`decomposeGEP`), a constant one stays a
//!    variable.
//! 3. `stripPointerCastsSameRepresentation` is the identity: MIR pointers are
//!    opaque, and a cast between address spaces is not a same-representation
//!    cast.
//! 4. `DebugCounter`, `DumpReproducers` (`generateReproducer`) and the
//!    `LLVM_DEBUG` output are not translated.
//!
//! # Example
//! `if (x < 5) { if (x < 10) ... }`: the inner compare is decided true.
//!
//! # Used by
//! Nothing yet: `difference` and `guards::Given::holds` move here.

use llrm_mir::constraintsystem::{ConstraintSystem, Entry, Row};
use llrm_mir::constantrange::ConstantRange;
use llrm_mir::context::signed;
use llrm_mir::hash::HashMap;
use llrm_mir::module::Operand;
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::known::{is_known_non_negative, is_known_positive};
use llrm_mir::valuetracking::pattern::{self, Side};
use llrm_mir::valuetracking::{MAX_ANALYSIS_RECURSION_DEPTH, Query};

/// `MaxConstraintValue`.
const MAX_CONSTRAINT_VALUE: i64 = i64::MAX;
/// `MinSignedConstraintValue`.
const MIN_SIGNED_CONSTRAINT_VALUE: i64 = i64::MIN;

// ---- CmpInst ------------------------------------------------------------

/// `CmpInst::isSigned`.
pub fn is_signed(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle)
}

/// `CmpInst::isUnsigned`.
pub fn is_unsigned(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Ugt | IntPredicate::Uge | IntPredicate::Ult | IntPredicate::Ule)
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
/// implies `nusw` and `nuw`.
fn gep_flags(flags: Flags) -> Flags {
    let mut out = Flags::default();
    if flags.contains(Flags::INBOUNDS) {
        out.insert(Flags::INBOUNDS);
        out.insert(Flags::NUSW);
        out.insert(Flags::NUW);
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
    let space = q.type_of(instruction.operands[0]).and_then(|ty| match q.context().types.get(ty) {
        llrm_mir::types::Type::Pointer(space) => Some(*space),
        _ => None,
    })?;
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
        if !can_collect_inner || variable.len() > 1 || variable2.len() > 1 || (!variable.is_empty() && !variable2.is_empty())
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
fn is_integer(
    q: &Query,
    s: Side,
) -> bool {
    match s {
        Side::Int(..) => true,
        Side::Value(v) => {
            matches!(q.type_of(v).map(|ty| q.context().types.get(ty)), Some(llrm_mir::types::Type::Int(_)))
        }
    }
}

fn is_pointer(
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
    matches!(s, Side::Int(bits, width) if bits == llrm_mir::context::mask(width))
        || matches!(s, Side::Value(v) if q.int_constant(v).is_some_and(|(bits, width)| bits == llrm_mir::context::mask(width)))
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
    let kind = if signed_wrap { llrm_mir::constantrange::NO_SIGNED_WRAP } else { llrm_mir::constantrange::NO_UNSIGNED_WRAP };
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
        return is_known_no_wrap_op(binary, Side::of(q, args[0]), Side::of(q, args[1]), Flags::default(), info, q, signed_wrap);
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
    if !matches!(opcode, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl) {
        return false;
    }
    is_known_no_wrap_op(
        opcode,
        Side::of(q, instruction.operands[0]),
        Side::of(q, instruction.operands[1]),
        instruction.flags,
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
        if q.layout.offset_bits(space) > 64 {
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

        let mut result =
            Decomposition::with(offsets.constant_offset, vec![DecompEntry { coefficient: 1, variable: offsets.base_ptr }]);
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
            if matches!(v, Side::Value(op) if matches!(q.instruction_of(op).map(|(_, i)| i.opcode.clone()), Some(Opcode::GetElementPtr { .. }))) {
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

        if !matches!(pred, IntPredicate::Ule | IntPredicate::Ult | IntPredicate::Sle | IntPredicate::Slt) {
            return ConstraintTy::default();
        }

        let is_signed_system = force_signed_system || is_signed(pred);
        // `stripPointerCastsSameRepresentation` is the identity (DEVIATE 3).
        let a_dec = self.decompose(q, op0, is_signed_system);
        let b_dec = self.decompose(q, op1, is_signed_system);
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
            let (ge_pred, le_pred) =
                if is_signed_system { (IntPredicate::Sge, IntPredicate::Sle) } else { (IntPredicate::Uge, IntPredicate::Ule) };
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
                dfs_in_stack.push(StackEntry { num_in, num_out, is_signed: r.is_signed, values_to_release: Vec::new() });
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
            | Opcode::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl | BinaryOp::Or | BinaryOp::Xor)
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
