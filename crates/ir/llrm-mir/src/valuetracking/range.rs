//! `computeConstantRange` and the select-pattern matcher it and
//! `computeKnownBits` use.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ValueTracking.cpp`
//! (`computeConstantRange`, `setLimitsForBinOp`, `getRangeForIntrinsic`,
//! `getRangeForSelectPattern`, `computeConstantRangeIncludingKnownBits`, the
//! `computeOverflowFor*` family and `matchSelectPattern` with its helpers),
//! each function linking the line it translates.
//!
//! A value the matcher builds (`getNotValue`, `lookThroughCast`'s folded
//! constant) is not in the module's context, which a query only reads: a
//! [`SelectOperand`] is an operand or such a constant, compared as LLVM
//! compares uniqued constants.
//!
//! DEVIATE (anything MIR cannot represent):
//! - vectors, scalable vectors, `undef`: the vector arms of
//!   `Constant::toConstantRange`, `shiftAmountKnownInRange` (7959-7980),
//!   `getFlippedStrictnessPredicateAndConstant` (9146-9228) and
//!   `lookThroughCastConst` (9394-9487).
//! - `half` is not a MIR type, so `setLimitForFPToI` never narrows (10738).
//! - `llvm.frexp`'s exponent range needs `computeKnownFPClass` (10790-10850).
//! - `vscale_range` is not a MIR attribute: `llvm.vscale` is `[1, 0)` (10640);
//!   the RISC-V `read_register` arm needs a triple (10644).
//! - `ConstantFoldCastOperand` folds integer, float and poison constants; a
//!   constant expression is left unfolded (9394-9487).
//! - `m_OneUse` of a constant condition is false: a constant's use list is not
//!   kept (`canConvertToMinOrMaxIntrinsic`, 9653).
//! - `SelectPatternFlavor` has no `FMinimum`/`FMaximum`/`Other`: LLVM has none.
//!
//! # Example
//! `urem i32 %x, 10` is in `[0, 10)`.

use super::context::{compute_known_bits_from_context, is_implied_by_dom_condition, is_valid_assume_for_context};
use super::guarantee::is_guaranteed_not_to_be_undef;
use super::known::compute_known_bits;
use super::pattern::{IntrinsicId, binary, intrinsic_id};
use super::query::Query;
use super::signbits::compute_num_sign_bits;
use crate::apint;
use crate::constantrange::{ConstantRange, OverflowResult as RangeOverflow, PreferredRangeType};
use crate::context::{ConstantKind, mask};
use crate::domtree::DomTree;
use crate::knownbits::KnownBits;
use crate::module::{BlockId, Function, GlobalKind, InstId, Instruction, MetadataOperand, Operand, Use, ValueDef};
use crate::opcode::{Attribute, BinaryOp, CastOp, Flags, FloatPredicate, IntPredicate, Opcode};
use crate::types::{FloatKind, Type, TypeId};

fn add(
    a: u128,
    b: u128,
    width: u32,
) -> u128 {
    a.wrapping_add(b) & mask(width)
}

fn sub(
    a: u128,
    b: u128,
    width: u32,
) -> u128 {
    a.wrapping_sub(b) & mask(width)
}

fn smin(width: u32) -> u128 {
    apint::sign_bit(width)
}

fn smax(width: u32) -> u128 {
    mask(width) >> 1
}

fn is_negative(
    a: u128,
    width: u32,
) -> bool {
    apint::bit(a, width - 1)
}

fn abs(
    a: u128,
    width: u32,
) -> u128 {
    if is_negative(a, width) { sub(0, a, width) } else { a }
}

// ---------------------------------------------------------------------------
// Overflow
// ---------------------------------------------------------------------------

/// `OverflowResult`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverflowResult {
    AlwaysOverflowsLow,
    AlwaysOverflowsHigh,
    MayOverflow,
    NeverOverflows,
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7700>
pub fn map_overflow_result(overflow: RangeOverflow) -> OverflowResult {
    match overflow {
        RangeOverflow::MayOverflow => OverflowResult::MayOverflow,
        RangeOverflow::AlwaysOverflowsLow => OverflowResult::AlwaysOverflowsLow,
        RangeOverflow::AlwaysOverflowsHigh => OverflowResult::AlwaysOverflowsHigh,
        RangeOverflow::NeverOverflows => OverflowResult::NeverOverflows,
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7715>
pub fn compute_constant_range_including_known_bits(
    q: &Query,
    v: Operand,
    for_signed: bool,
) -> ConstantRange {
    let cr1 = ConstantRange::from_known_bits(&compute_known_bits(q, v, 0), for_signed);
    let cr2 = compute_constant_range(q, v, for_signed, 0);
    let range_type = if for_signed { PreferredRangeType::Signed } else { PreferredRangeType::Unsigned };
    cr1.intersect_with(&cr2, range_type)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7727>
pub fn compute_overflow_for_unsigned_mul(
    q: &Query,
    lhs: Operand,
    rhs: Operand,
    is_nsw: bool,
) -> OverflowResult {
    let lhs_range = compute_constant_range_including_known_bits(q, lhs, false);
    let rhs_range = compute_constant_range_including_known_bits(q, rhs, false);
    if is_nsw && lhs_range.is_all_non_negative() && rhs_range.is_all_non_negative() {
        return OverflowResult::NeverOverflows;
    }
    map_overflow_result(lhs_range.unsigned_mul_may_overflow(&rhs_range))
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7743>
pub fn compute_overflow_for_signed_mul(
    q: &Query,
    lhs: Operand,
    rhs: Operand,
) -> OverflowResult {
    let bit_width = q.scalar_bits(lhs);
    let sign_bits = compute_num_sign_bits(q, lhs, 0) + compute_num_sign_bits(q, rhs, 0);
    if sign_bits > bit_width + 1 {
        return OverflowResult::NeverOverflows;
    }
    if sign_bits == bit_width + 1 {
        let lhs_known = compute_known_bits(q, lhs, 0);
        let rhs_known = compute_known_bits(q, rhs, 0);
        if lhs_known.is_non_negative() || rhs_known.is_non_negative() {
            return OverflowResult::NeverOverflows;
        }
    }
    OverflowResult::MayOverflow
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7782>
pub fn compute_overflow_for_unsigned_add(
    q: &Query,
    lhs: Operand,
    rhs: Operand,
) -> OverflowResult {
    let lhs_range = compute_constant_range_including_known_bits(q, lhs, false);
    let rhs_range = compute_constant_range_including_known_bits(q, rhs, false);
    map_overflow_result(lhs_range.unsigned_add_may_overflow(&rhs_range))
}

/// The static `computeOverflowForSignedAdd`, where `add` is the `AddOperator`
/// the operands are of, if any.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7793>
fn compute_overflow_for_signed_add_impl(
    q: &Query,
    lhs: Operand,
    rhs: Operand,
    add: Option<InstId>,
) -> OverflowResult {
    if let Some(add) = add
        && q.function.instruction(add).flags.contains(Flags::NSW)
    {
        return OverflowResult::NeverOverflows;
    }
    if compute_num_sign_bits(q, lhs, 0) > 1 && compute_num_sign_bits(q, rhs, 0) > 1 {
        return OverflowResult::NeverOverflows;
    }
    let lhs_range = compute_constant_range_including_known_bits(q, lhs, true);
    let rhs_range = compute_constant_range_including_known_bits(q, rhs, true);
    let overflow = map_overflow_result(lhs_range.signed_add_may_overflow(&rhs_range));
    if overflow != OverflowResult::MayOverflow {
        return overflow;
    }
    let Some(add) = add else { return OverflowResult::MayOverflow };
    let lhs_or_rhs_known_non_negative = lhs_range.is_all_non_negative() || rhs_range.is_all_non_negative();
    let lhs_or_rhs_known_negative = lhs_range.is_all_negative() || rhs_range.is_all_negative();
    if lhs_or_rhs_known_non_negative || lhs_or_rhs_known_negative {
        let mut add_known = KnownBits::unknown(lhs_range.width);
        let result = q.function.instruction(add).result.expect("an add has a result");
        compute_known_bits_from_context(q, Operand::Value(result), &mut add_known, 0);
        if (add_known.is_non_negative() && lhs_or_rhs_known_non_negative)
            || (add_known.is_negative() && lhs_or_rhs_known_negative)
        {
            return OverflowResult::NeverOverflows;
        }
    }
    OverflowResult::MayOverflow
}

/// `computeOverflowForSignedAdd(const AddOperator *Add, ...)`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8359>
pub fn compute_overflow_for_signed_add_operator(
    q: &Query,
    add: InstId,
) -> OverflowResult {
    let operands = &q.function.instruction(add).operands;
    compute_overflow_for_signed_add_impl(q, operands[0], operands[1], Some(add))
}

/// `computeOverflowForSignedAdd(LHS, RHS, ...)`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8365>
pub fn compute_overflow_for_signed_add(
    q: &Query,
    lhs: Operand,
    rhs: Operand,
) -> OverflowResult {
    compute_overflow_for_signed_add_impl(q, lhs, rhs, None)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7851>
pub fn compute_overflow_for_unsigned_sub(
    q: &Query,
    lhs: Operand,
    rhs: Operand,
) -> OverflowResult {
    let urem = binary(q, rhs, BinaryOp::URem).is_some_and(|(_, _, a, _)| a == lhs);
    let nuw_sub =
        binary(q, rhs, BinaryOp::Sub).is_some_and(|(_, one, a, _)| a == lhs && one.flags.contains(Flags::NUW));
    if (urem || nuw_sub) && is_guaranteed_not_to_be_undef(q, lhs, 0) {
        return OverflowResult::NeverOverflows;
    }
    if let Some(cxt) = q.cxt
        && let Some(implied) = is_implied_by_dom_condition(q, IntPredicate::Uge, lhs, rhs, cxt)
    {
        return if implied { OverflowResult::NeverOverflows } else { OverflowResult::AlwaysOverflowsLow };
    }
    let lhs_range = compute_constant_range_including_known_bits(q, lhs, false);
    let rhs_range = compute_constant_range_including_known_bits(q, rhs, false);
    map_overflow_result(lhs_range.unsigned_sub_may_overflow(&rhs_range))
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7884>
pub fn compute_overflow_for_signed_sub(
    q: &Query,
    lhs: Operand,
    rhs: Operand,
) -> OverflowResult {
    let srem = binary(q, rhs, BinaryOp::SRem).is_some_and(|(_, _, a, _)| a == lhs);
    let nsw_sub =
        binary(q, rhs, BinaryOp::Sub).is_some_and(|(_, one, a, _)| a == lhs && one.flags.contains(Flags::NSW));
    if (srem || nsw_sub) && is_guaranteed_not_to_be_undef(q, lhs, 0) {
        return OverflowResult::NeverOverflows;
    }
    if compute_num_sign_bits(q, lhs, 0) > 1 && compute_num_sign_bits(q, rhs, 0) > 1 {
        return OverflowResult::NeverOverflows;
    }
    let lhs_range = compute_constant_range_including_known_bits(q, lhs, true);
    let rhs_range = compute_constant_range_including_known_bits(q, rhs, true);
    map_overflow_result(lhs_range.signed_sub_may_overflow(&rhs_range))
}

/// `DominatorTree::dominates(BasicBlockEdge, BasicBlock)`, the edge `start` to
/// `end`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Dominators.cpp#L162>
fn edge_dominates_block(
    function: &Function,
    dt: &DomTree,
    start: BlockId,
    end: BlockId,
    use_block: BlockId,
) -> bool {
    if !dt.dominates(end, use_block) {
        return false;
    }
    let edges: Vec<&Use> =
        function.block_users(end).iter().filter(|one| function.instruction(one.user).opcode.is_terminator()).collect();
    if edges.len() == 1 {
        return true;
    }
    let mut duplicate = 0;
    for edge in edges {
        let Some(from) = function.parent(edge.user) else { continue };
        if from == start {
            duplicate += 1;
            if duplicate > 1 {
                return false;
            }
            continue;
        }
        if !dt.dominates(end, from) {
            return false;
        }
    }
    true
}

/// `DominatorTree::dominates(BasicBlockEdge, Use)`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Dominators.cpp#L213>
pub fn edge_dominates_use(
    function: &Function,
    dt: &DomTree,
    start: BlockId,
    end: BlockId,
    used: &Use,
) -> bool {
    let user = function.instruction(used.user);
    let phi = user.opcode == Opcode::Phi;
    let incoming = || match user.operands.get(used.index as usize + 1) {
        Some(Operand::Block(block)) => Some(*block),
        _ => None,
    };
    if phi && function.parent(used.user) == Some(end) && incoming() == Some(start) {
        return true;
    }
    let Some(use_block) = (if phi { incoming() } else { function.parent(used.user) }) else { return false };
    edge_dominates_block(function, dt, start, end, use_block)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7912>
pub fn is_overflow_intrinsic_no_wrap(
    q: &Query,
    wo: InstId,
    dt: &DomTree,
) -> bool {
    let function = q.function;
    let Some(wo_result) = function.instruction(wo).result else { return true };
    let mut guarding_branches: Vec<InstId> = Vec::new();
    let mut results: Vec<InstId> = Vec::new();
    for used in function.users(wo_result) {
        let user = function.instruction(used.user);
        let Opcode::ExtractValue(indices) = &user.opcode else { return false };
        if indices[0] == 0 {
            results.push(used.user);
        } else if let Some(result) = user.result {
            for branch in function.users(result) {
                let candidate = function.instruction(branch.user);
                if candidate.opcode == Opcode::Br && candidate.operands.len() == 3 {
                    guarding_branches.push(branch.user);
                }
            }
        }
    }
    let all_uses_guarded_by_branch = |branch: &InstId| {
        let instruction = function.instruction(*branch);
        let (Some(start), Operand::Block(end)) = (function.parent(*branch), instruction.operands[2]) else {
            return false;
        };
        for result in &results {
            if let Some(block) = function.parent(*result)
                && edge_dominates_block(function, dt, start, end, block)
            {
                continue;
            }
            let Some(value) = function.instruction(*result).result else { continue };
            for used in function.users(value) {
                if !edge_dominates_use(function, dt, start, end, used) {
                    return false;
                }
            }
        }
        true
    };
    guarding_branches.iter().any(all_uses_guarded_by_branch)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7959>
pub fn shift_amount_known_in_range(
    q: &Query,
    shift_amount: Operand,
) -> bool {
    q.int_constant(shift_amount).is_some_and(|(bits, width)| bits < u128::from(width))
}

// ---------------------------------------------------------------------------
// Select patterns
// ---------------------------------------------------------------------------

/// `SelectPatternFlavor`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectPatternFlavor {
    /// `SPF_UNKNOWN`.
    None,
    SMin,
    UMin,
    SMax,
    UMax,
    FMinNum,
    FMaxNum,
    Abs,
    NAbs,
}

impl SelectPatternFlavor {
    /// `SelectPatternResult::isMinOrMax`.
    pub fn is_min_or_max(self) -> bool {
        !matches!(self, Self::None | Self::Abs | Self::NAbs)
    }
}

/// `SelectPatternNaNBehavior`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectPatternNaNBehavior {
    /// `SPNB_NA`: not applicable (an integer pattern).
    NA,
    ReturnsNaN,
    ReturnsOther,
    ReturnsAny,
}

/// A value of the select-pattern matcher: an operand, or a constant it built
/// (see the header).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectOperand {
    Value(Operand),
    Int { ty: TypeId, bits: u128 },
    Float { ty: TypeId, bits: u64 },
    Poison(TypeId),
}

impl From<Operand> for SelectOperand {
    fn from(operand: Operand) -> Self {
        Self::Value(operand)
    }
}

impl SelectOperand {
    /// The operand, if it is one the module has.
    pub fn operand(self) -> Option<Operand> {
        match self {
            Self::Value(operand) => Some(operand),
            _ => None,
        }
    }
}

/// `CmpInst::Predicate`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Predicate {
    Int(IntPredicate),
    Float(FloatPredicate),
}

impl Predicate {
    fn is_fp(self) -> bool {
        matches!(self, Self::Float(_))
    }

    /// `CmpInst::getSwappedPredicate`.
    fn swapped(self) -> Self {
        use FloatPredicate::*;
        match self {
            Self::Int(pred) => Self::Int(pred.swapped()),
            Self::Float(pred) => Self::Float(match pred {
                Ogt => Olt,
                Oge => Ole,
                Olt => Ogt,
                Ole => Oge,
                Ugt => Ult,
                Uge => Ule,
                Ult => Ugt,
                Ule => Uge,
                same => same,
            }),
        }
    }

    /// `CmpInst::getInversePredicate`.
    fn inverse(self) -> Self {
        use FloatPredicate::*;
        match self {
            Self::Int(pred) => Self::Int(pred.inverse()),
            Self::Float(pred) => Self::Float(match pred {
                False => True,
                True => False,
                Oeq => Une,
                Une => Oeq,
                Ogt => Ule,
                Ule => Ogt,
                Oge => Ult,
                Ult => Oge,
                Olt => Uge,
                Uge => Olt,
                Ole => Ugt,
                Ugt => Ole,
                One => Ueq,
                Ueq => One,
                Ord => Uno,
                Uno => Ord,
            }),
        }
    }

    /// `CmpInst::isOrdered`.
    fn is_ordered(self) -> bool {
        use FloatPredicate::*;
        matches!(self, Self::Float(Oeq | Ogt | Oge | Olt | Ole | One | Ord))
    }

    /// `CmpInst::isEquality`.
    fn is_equality(self) -> bool {
        use FloatPredicate::*;
        match self {
            Self::Int(pred) => matches!(pred, IntPredicate::Eq | IntPredicate::Ne),
            Self::Float(pred) => matches!(pred, Oeq | One | Ueq | Une),
        }
    }

    /// `CmpInst::isSigned`.
    fn is_signed(self) -> bool {
        matches!(
            self,
            Self::Int(IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle)
        )
    }

    /// `CmpInst::isUnsigned`.
    fn is_unsigned(self) -> bool {
        matches!(
            self,
            Self::Int(IntPredicate::Ugt | IntPredicate::Uge | IntPredicate::Ult | IntPredicate::Ule)
        )
    }
}

/// `FastMathFlags`, the two the matcher reads.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FastMathFlags {
    pub no_nans: bool,
    pub no_signed_zeros: bool,
}

/// `SelectPatternResult`, and the `LHS`, `RHS` and `CastOp` the C++ returns
/// through its arguments: set as the C++ sets them, and for a pattern LLVM
/// does not find, whatever it last assigned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectPatternResult {
    pub flavor: SelectPatternFlavor,
    pub nan_behavior: SelectPatternNaNBehavior,
    pub ordered: bool,
    pub lhs: Option<SelectOperand>,
    pub rhs: Option<SelectOperand>,
    pub cast_op: Option<CastOp>,
}

impl SelectPatternResult {
    fn new(
        flavor: SelectPatternFlavor,
        nan_behavior: SelectPatternNaNBehavior,
        ordered: bool,
    ) -> Self {
        Self { flavor, nan_behavior, ordered, lhs: None, rhs: None, cast_op: None }
    }

    /// `{SPF_UNKNOWN, SPNB_NA, false}`.
    fn unknown() -> Self {
        Self::new(SelectPatternFlavor::None, SelectPatternNaNBehavior::NA, false)
    }

    /// `{Flavor, SPNB_NA, false}`.
    fn flavor(flavor: SelectPatternFlavor) -> Self {
        Self::new(flavor, SelectPatternNaNBehavior::NA, false)
    }

    fn with(
        self,
        lhs: Option<SelectOperand>,
        rhs: Option<SelectOperand>,
    ) -> Self {
        Self { lhs, rhs, ..self }
    }
}

/// `v` as LLVM compares it: a uniqued constant is one value however it is
/// reached.
fn canon(
    q: &Query,
    v: SelectOperand,
) -> SelectOperand {
    let SelectOperand::Value(Operand::Constant(id)) = v else { return v };
    let constant = q.context.get(id);
    match constant.kind {
        ConstantKind::Int(bits) => SelectOperand::Int { ty: constant.ty, bits },
        ConstantKind::Zero | ConstantKind::Null if q.context.types.int_bits(constant.ty).is_some() => {
            SelectOperand::Int { ty: constant.ty, bits: 0 }
        }
        ConstantKind::Float(bits) => SelectOperand::Float { ty: constant.ty, bits },
        ConstantKind::Poison => SelectOperand::Poison(constant.ty),
        _ => v,
    }
}

fn same(
    q: &Query,
    a: SelectOperand,
    b: SelectOperand,
) -> bool {
    canon(q, a) == canon(q, b)
}

fn type_of(
    q: &Query,
    v: SelectOperand,
) -> Option<TypeId> {
    match v {
        SelectOperand::Value(operand) => q.type_of(operand),
        SelectOperand::Int { ty, .. } | SelectOperand::Float { ty, .. } | SelectOperand::Poison(ty) => Some(ty),
    }
}

/// `m_APInt`: the integer constant `v` is, and its width.
pub(super) fn m_apint(
    q: &Query,
    v: SelectOperand,
) -> Option<(u128, u32)> {
    match canon(q, v) {
        SelectOperand::Int { ty, bits } => Some((bits, q.context.types.int_bits(ty)?)),
        _ => None,
    }
}

fn m_zero_int(
    q: &Query,
    v: SelectOperand,
) -> bool {
    m_apint(q, v).is_some_and(|(bits, _)| bits == 0)
}

fn m_all_ones(
    q: &Query,
    v: SelectOperand,
) -> bool {
    m_apint(q, v).is_some_and(|(bits, width)| bits == mask(width))
}

fn m_one(
    q: &Query,
    v: SelectOperand,
) -> bool {
    m_apint(q, v).is_some_and(|(bits, _)| bits == 1)
}

fn instruction_of<'a>(
    q: &Query<'a>,
    v: SelectOperand,
) -> Option<(InstId, &'a Instruction)> {
    q.instruction_of(v.operand()?)
}

/// `m_BinOp(L, R)` of `op`: the instruction and both operands.
fn m_binary<'a>(
    q: &Query<'a>,
    v: SelectOperand,
    op: BinaryOp,
) -> Option<(&'a Instruction, SelectOperand, SelectOperand)> {
    let (_, instruction) = instruction_of(q, v)?;
    (matches!(instruction.opcode, Opcode::Binary(kind) if kind == op))
        .then(|| (instruction, instruction.operands[0].into(), instruction.operands[1].into()))
}

/// `m_Neg(V)`: `sub 0, V`.
fn m_neg<'a>(
    q: &Query<'a>,
    v: SelectOperand,
) -> Option<(&'a Instruction, SelectOperand)> {
    let (instruction, zero, inner) = m_binary(q, v, BinaryOp::Sub)?;
    m_zero_int(q, zero).then_some((instruction, inner))
}

/// `m_Not(V)`: `xor -1, V` in either order.
fn m_not(
    q: &Query,
    v: SelectOperand,
) -> Option<SelectOperand> {
    let (_, a, b) = m_binary(q, v, BinaryOp::Xor)?;
    if m_all_ones(q, a) {
        Some(b)
    } else if m_all_ones(q, b) {
        Some(a)
    } else {
        None
    }
}

/// `m_SExt(V)`.
fn m_sext(
    q: &Query,
    v: SelectOperand,
) -> Option<SelectOperand> {
    let (_, instruction) = instruction_of(q, v)?;
    (matches!(instruction.opcode, Opcode::Cast(CastOp::SExt))).then(|| instruction.operands[0].into())
}

/// `m_Intrinsic<id>(...)`: the call's arguments.
fn m_intrinsic<'a>(
    q: &Query<'a>,
    v: SelectOperand,
    id: IntrinsicId,
) -> Option<&'a [Operand]> {
    let (inst, instruction) = instruction_of(q, v)?;
    if !matches!(instruction.opcode, Opcode::Call(_)) || intrinsic_id(q, inst) != Some(id) {
        return None;
    }
    instruction.operands.split_last().map(|(_, arguments)| arguments)
}

/// `ConstantFP`'s value, as a double: a `float` widens exactly.
fn fp_value(
    q: &Query,
    v: SelectOperand,
) -> Option<f64> {
    let SelectOperand::Float { ty, bits } = canon(q, v) else { return None };
    match q.context.types.get(ty) {
        Type::Float(FloatKind::Float) => Some(f64::from(f32::from_bits(bits as u32))),
        Type::Float(_) => Some(f64::from_bits(bits)),
        _ => None,
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8740>
fn is_known_non_nan(
    q: &Query,
    v: SelectOperand,
    fmf: FastMathFlags,
) -> bool {
    fmf.no_nans || fp_value(q, v).is_some_and(|value| !value.is_nan())
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8763>
fn is_known_non_zero_fp(
    q: &Query,
    v: SelectOperand,
) -> bool {
    fp_value(q, v).is_some_and(|value| value != 0.0)
}

/// `m_AnyZeroFP`.
fn m_any_zero_fp(
    q: &Query,
    v: SelectOperand,
) -> bool {
    fp_value(q, v).is_some_and(|value| value == 0.0)
}

/// `m_OrdOrUnordFMax` (`max`) or `m_OrdOrUnordFMin`: a select of an `fcmp`'s
/// two operands that is that max or min. The `fcmp`'s operands.
fn m_ord_or_unord_fmax_min(
    q: &Query,
    v: SelectOperand,
    max: bool,
) -> Option<(SelectOperand, SelectOperand)> {
    let (_, select) = instruction_of(q, v)?;
    if select.opcode != Opcode::Select {
        return None;
    }
    let (_, cmp) = q.instruction_of(select.operands[0])?;
    let Opcode::FCmp(pred) = cmp.opcode else { return None };
    let (true_val, false_val) = (SelectOperand::from(select.operands[1]), SelectOperand::from(select.operands[2]));
    let (lhs, rhs) = (SelectOperand::from(cmp.operands[0]), SelectOperand::from(cmp.operands[1]));
    if (!same(q, true_val, lhs) || !same(q, false_val, rhs)) && (!same(q, true_val, rhs) || !same(q, false_val, lhs)) {
        return None;
    }
    let pred = if same(q, lhs, true_val) { Predicate::Float(pred) } else { Predicate::Float(pred).inverse() };
    use FloatPredicate::*;
    let class = if max {
        matches!(pred, Predicate::Float(Ogt | Oge | Ugt | Uge))
    } else {
        matches!(pred, Predicate::Float(Olt | Ole | Ult | Ule))
    };
    class.then_some((lhs, rhs))
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8784>
fn match_fast_float_clamp(
    q: &Query,
    mut pred: Predicate,
    cmp_lhs: SelectOperand,
    cmp_rhs: SelectOperand,
    mut true_val: SelectOperand,
    mut false_val: SelectOperand,
) -> SelectPatternResult {
    if same(q, cmp_rhs, false_val) {
        std::mem::swap(&mut true_val, &mut false_val);
        pred = pred.inverse();
    }
    let result = |r: SelectPatternResult| r.with(Some(true_val), Some(false_val));
    let unknown = || result(SelectPatternResult::unknown());
    let fc1 = fp_value(q, cmp_rhs).filter(|value| value.is_finite());
    let Some(fc1) = fc1.filter(|_| same(q, cmp_rhs, true_val)) else { return unknown() };
    let returns_any = |flavor| result(SelectPatternResult::new(flavor, SelectPatternNaNBehavior::ReturnsAny, false));
    use FloatPredicate::*;
    match pred {
        Predicate::Float(Olt | Ole | Ult | Ule) => {
            if let Some((lhs, rhs)) = m_ord_or_unord_fmax_min(q, false_val, false)
                && same(q, lhs, cmp_lhs)
                && fp_value(q, rhs).is_some_and(|fc2| fc1 < fc2)
            {
                return returns_any(SelectPatternFlavor::FMaxNum);
            }
            if let Some(arguments) = m_intrinsic(q, false_val, IntrinsicId::Minnum)
                && arguments.len() >= 2
                && same(q, arguments[0].into(), cmp_lhs)
                && fp_value(q, arguments[1].into()).is_some_and(|fc2| fc1 < fc2)
            {
                return returns_any(SelectPatternFlavor::FMaxNum);
            }
        }
        Predicate::Float(Ogt | Oge | Ugt | Uge) => {
            if let Some((lhs, rhs)) = m_ord_or_unord_fmax_min(q, false_val, true)
                && same(q, lhs, cmp_lhs)
                && fp_value(q, rhs).is_some_and(|fc2| fc1 > fc2)
            {
                return returns_any(SelectPatternFlavor::FMinNum);
            }
            if let Some(arguments) = m_intrinsic(q, false_val, IntrinsicId::Maxnum)
                && arguments.len() >= 2
                && same(q, arguments[0].into(), cmp_lhs)
                && fp_value(q, arguments[1].into()).is_some_and(|fc2| fc1 > fc2)
            {
                return returns_any(SelectPatternFlavor::FMinNum);
            }
        }
        _ => {}
    }
    unknown()
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8840>
fn match_clamp(
    q: &Query,
    mut pred: IntPredicate,
    cmp_lhs: SelectOperand,
    cmp_rhs: SelectOperand,
    mut true_val: SelectOperand,
    mut false_val: SelectOperand,
) -> SelectPatternResult {
    if !same(q, cmp_rhs, true_val) {
        pred = pred.swapped();
        std::mem::swap(&mut true_val, &mut false_val);
    }
    if same(q, cmp_rhs, true_val)
        && let Some((c1, width)) = m_apint(q, cmp_rhs)
    {
        let with_c2 = |id: IntrinsicId| {
            let arguments = m_intrinsic(q, false_val, id)?;
            if arguments.len() < 2 || !same(q, arguments[0].into(), cmp_lhs) {
                return None;
            }
            m_apint(q, arguments[1].into()).map(|(c2, _)| c2)
        };
        let s1 = apint::signed(c1, width);
        if let Some(c2) = with_c2(IntrinsicId::Smin)
            && s1 < apint::signed(c2, width)
            && pred == IntPredicate::Slt
        {
            return SelectPatternResult::flavor(SelectPatternFlavor::SMax);
        }
        if let Some(c2) = with_c2(IntrinsicId::Smax)
            && s1 > apint::signed(c2, width)
            && pred == IntPredicate::Sgt
        {
            return SelectPatternResult::flavor(SelectPatternFlavor::SMin);
        }
        if let Some(c2) = with_c2(IntrinsicId::Umin)
            && c1 < c2
            && pred == IntPredicate::Ult
        {
            return SelectPatternResult::flavor(SelectPatternFlavor::UMax);
        }
        if let Some(c2) = with_c2(IntrinsicId::Umax)
            && c1 > c2
            && pred == IntPredicate::Ugt
        {
            return SelectPatternResult::flavor(SelectPatternFlavor::UMin);
        }
    }
    SelectPatternResult::unknown()
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8876>
fn match_min_max_of_min_max(
    q: &Query,
    mut pred: IntPredicate,
    mut cmp_lhs: SelectOperand,
    mut cmp_rhs: SelectOperand,
    t_val: SelectOperand,
    f_val: SelectOperand,
    depth: u32,
) -> SelectPatternResult {
    use IntPredicate::*;
    let l = match_select_pattern_cast(q, t_val, false, depth + 1);
    if !l.flavor.is_min_or_max() {
        return SelectPatternResult::unknown();
    }
    let (Some(a), Some(b)) = (l.lhs, l.rhs) else { return SelectPatternResult::unknown() };
    let r = match_select_pattern_cast(q, f_val, false, depth + 1);
    if l.flavor != r.flavor {
        return SelectPatternResult::unknown();
    }
    let (Some(c), Some(d)) = (r.lhs, r.rhs) else { return SelectPatternResult::unknown() };

    let (swap_if, keep): (&[IntPredicate], &[IntPredicate]) = match l.flavor {
        SelectPatternFlavor::SMin => (&[Sgt, Sge], &[Slt, Sle]),
        SelectPatternFlavor::SMax => (&[Slt, Sle], &[Sgt, Sge]),
        SelectPatternFlavor::UMin => (&[Ugt, Uge], &[Ult, Ule]),
        SelectPatternFlavor::UMax => (&[Ult, Ule], &[Ugt, Uge]),
        _ => return SelectPatternResult::unknown(),
    };
    if swap_if.contains(&pred) {
        pred = pred.swapped();
        std::mem::swap(&mut cmp_lhs, &mut cmp_rhs);
    }
    if !keep.contains(&pred) {
        return SelectPatternResult::unknown();
    }

    let not_of = |value: SelectOperand, of: SelectOperand| m_not(q, value).is_some_and(|inner| same(q, inner, of));
    if same(q, d, b) && ((same(q, cmp_lhs, a) && same(q, cmp_rhs, c)) || (not_of(c, cmp_lhs) && not_of(a, cmp_rhs))) {
        return SelectPatternResult::flavor(l.flavor);
    }
    if same(q, c, b) && ((same(q, cmp_lhs, a) && same(q, cmp_rhs, d)) || (not_of(d, cmp_lhs) && not_of(a, cmp_rhs))) {
        return SelectPatternResult::flavor(l.flavor);
    }
    if same(q, d, a) && ((same(q, cmp_lhs, b) && same(q, cmp_rhs, c)) || (not_of(c, cmp_lhs) && not_of(b, cmp_rhs))) {
        return SelectPatternResult::flavor(l.flavor);
    }
    if same(q, c, a) && ((same(q, cmp_lhs, b) && same(q, cmp_rhs, d)) || (not_of(d, cmp_lhs) && not_of(b, cmp_rhs))) {
        return SelectPatternResult::flavor(l.flavor);
    }
    SelectPatternResult::unknown()
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8972>
pub fn get_not_value(
    q: &Query,
    v: SelectOperand,
) -> Option<SelectOperand> {
    if let Some(inner) = m_not(q, v) {
        return Some(inner);
    }
    let (bits, width) = m_apint(q, v)?;
    Some(SelectOperand::Int { ty: type_of(q, v)?, bits: !bits & mask(width) })
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8985>
fn match_min_max(
    q: &Query,
    pred: IntPredicate,
    cmp_lhs: SelectOperand,
    cmp_rhs: SelectOperand,
    true_val: SelectOperand,
    false_val: SelectOperand,
    depth: u32,
) -> SelectPatternResult {
    let with = |r: SelectPatternResult| r.with(Some(true_val), Some(false_val));
    let spr = match_clamp(q, pred, cmp_lhs, cmp_rhs, true_val, false_val);
    if spr.flavor != SelectPatternFlavor::None {
        return with(spr);
    }
    let spr = match_min_max_of_min_max(q, pred, cmp_lhs, cmp_rhs, true_val, false_val, depth);
    if spr.flavor != SelectPatternFlavor::None {
        return with(spr);
    }

    use IntPredicate::*;
    let not_true = get_not_value(q, true_val);
    let not_false = get_not_value(q, false_val);
    if not_true.is_some_and(|n| same(q, cmp_lhs, n)) && not_false.is_some_and(|n| same(q, cmp_rhs, n)) {
        match pred {
            Sgt => return with(SelectPatternResult::flavor(SelectPatternFlavor::SMin)),
            Slt => return with(SelectPatternResult::flavor(SelectPatternFlavor::SMax)),
            Ugt => return with(SelectPatternResult::flavor(SelectPatternFlavor::UMin)),
            Ult => return with(SelectPatternResult::flavor(SelectPatternFlavor::UMax)),
            _ => {}
        }
    }
    if not_false.is_some_and(|n| same(q, cmp_lhs, n)) && not_true.is_some_and(|n| same(q, cmp_rhs, n)) {
        match pred {
            Sgt => return with(SelectPatternResult::flavor(SelectPatternFlavor::SMax)),
            Slt => return with(SelectPatternResult::flavor(SelectPatternFlavor::SMin)),
            Ugt => return with(SelectPatternResult::flavor(SelectPatternFlavor::UMax)),
            Ult => return with(SelectPatternResult::flavor(SelectPatternFlavor::UMin)),
            _ => {}
        }
    }

    if pred != Sgt && pred != Slt {
        return with(SelectPatternResult::unknown());
    }
    let Some((c1, width)) = m_apint(q, cmp_rhs) else { return with(SelectPatternResult::unknown()) };

    let lhs_is_true = same(q, cmp_lhs, true_val);
    let lhs_is_false = same(q, cmp_lhs, false_val);
    let c2 = if lhs_is_true { m_apint(q, false_val) } else { None }
        .or_else(|| if lhs_is_false { m_apint(q, true_val) } else { None });
    if let Some((c2, _)) = c2 {
        if pred == Slt && c1 == 0 && c2 == smax(width) {
            let flavor = if lhs_is_true { SelectPatternFlavor::UMax } else { SelectPatternFlavor::UMin };
            return with(SelectPatternResult::flavor(flavor));
        }
        if pred == Sgt && c1 == mask(width) && c2 == smin(width) {
            let flavor = if lhs_is_false { SelectPatternFlavor::UMax } else { SelectPatternFlavor::UMin };
            return with(SelectPatternResult::flavor(flavor));
        }
    }
    with(SelectPatternResult::unknown())
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9054>
pub fn is_known_negation(
    q: &Query,
    x: SelectOperand,
    y: SelectOperand,
    need_nsw: bool,
    allow_poison: bool,
) -> bool {
    let is_negation_of = |x: SelectOperand, y: SelectOperand| {
        let Some((instruction, inner)) = m_neg(q, x) else { return false };
        if !same(q, inner, y) {
            return false;
        }
        if need_nsw && !instruction.flags.contains(Flags::NSW) {
            return false;
        }
        // The zero is a scalar integer, so `isNullValue` holds.
        let _ = allow_poison;
        true
    };
    if is_negation_of(x, y) || is_negation_of(y, x) {
        return true;
    }
    let sub_of = |v: SelectOperand| {
        m_binary(q, v, BinaryOp::Sub).filter(|(instruction, ..)| !need_nsw || instruction.flags.contains(Flags::NSW))
    };
    sub_of(x).is_some_and(|(_, a, b)| sub_of(y).is_some_and(|(_, y0, y1)| same(q, y0, b) && same(q, y1, a)))
}

/// `m_ICmp(Pred, L, R)`: the `icmp`'s predicate, `samesign` and operands.
fn m_icmp(
    q: &Query,
    v: Operand,
) -> Option<(IntPredicate, bool, Operand, Operand)> {
    let (_, instruction) = q.instruction_of(v)?;
    let Opcode::ICmp(pred) = instruction.opcode else { return None };
    Some((pred, instruction.flags.contains(Flags::SAMESIGN), instruction.operands[0], instruction.operands[1]))
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9085>
pub fn is_known_inversion(
    q: &Query,
    x: Operand,
    y: Operand,
) -> bool {
    let Some((pred1, same_sign1, a, b)) = m_icmp(q, x) else { return false };
    // `m_c_ICmp(Pred2, m_Specific(A), m_Value(C))`.
    let Some((pred2, same_sign2, c)) = m_icmp(q, y).and_then(|(pred, same_sign, l, r)| {
        if same(q, l.into(), a.into()) {
            Some((pred, same_sign, r))
        } else if same(q, r.into(), a.into()) {
            Some((pred.swapped(), same_sign, l))
        } else {
            None
        }
    }) else {
        return false;
    };
    if same_sign1 != same_sign2 {
        return false;
    }
    if same(q, b.into(), c.into()) {
        return pred1 == pred2.inverse();
    }
    let (Some((rhs1, width)), Some((rhs2, _))) = (m_apint(q, b.into()), m_apint(q, c.into())) else { return false };
    if same_sign1 && is_negative(rhs1, width) != is_negative(rhs2, width) {
        return false;
    }
    let cr1 = ConstantRange::make_exact_icmp_region(pred1, width, rhs1);
    let cr2 = ConstantRange::make_exact_icmp_region(pred2, width, rhs2);
    cr1.inverse() == cr2
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9115>
pub fn get_select_pattern(
    pred: Predicate,
    nan_behavior: SelectPatternNaNBehavior,
    ordered: bool,
) -> SelectPatternResult {
    use FloatPredicate as F;
    use IntPredicate as I;
    use SelectPatternFlavor::*;
    match pred {
        Predicate::Int(I::Ugt | I::Uge) => SelectPatternResult::flavor(UMax),
        Predicate::Int(I::Sgt | I::Sge) => SelectPatternResult::flavor(SMax),
        Predicate::Int(I::Ult | I::Ule) => SelectPatternResult::flavor(UMin),
        Predicate::Int(I::Slt | I::Sle) => SelectPatternResult::flavor(SMin),
        Predicate::Float(F::Ugt | F::Uge | F::Ogt | F::Oge) => SelectPatternResult::new(FMaxNum, nan_behavior, ordered),
        Predicate::Float(F::Ult | F::Ule | F::Olt | F::Ole) => SelectPatternResult::new(FMinNum, nan_behavior, ordered),
        _ => SelectPatternResult::unknown(),
    }
}

/// `getFlippedStrictnessPredicateAndConstant` of an integer scalar: the new
/// predicate, its `samesign`, and the constant plus or minus one.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9146>
pub fn get_flipped_strictness_predicate_and_constant(
    q: &Query,
    pred: IntPredicate,
    same_sign: bool,
    c: SelectOperand,
) -> Option<(IntPredicate, bool, SelectOperand)> {
    use IntPredicate::*;
    let SelectOperand::Int { ty, bits } = canon(q, c) else { return None };
    let width = q.context.types.int_bits(ty)?;
    let is_signed = Predicate::Int(pred).is_signed();
    let unsigned_pred = match pred {
        Sgt => Ugt,
        Sge => Uge,
        Slt => Ult,
        Sle => Ule,
        other => other,
    };
    let will_increment = matches!(unsigned_pred, Ule | Ugt);
    let is_max_value = |signed: bool| bits == if signed { smax(width) } else { mask(width) };
    let is_min_value = |signed: bool| bits == if signed { smin(width) } else { 0 };
    let constant_is_ok = if will_increment { !is_max_value(is_signed) } else { !is_min_value(is_signed) }
        && (!same_sign || if will_increment { !is_max_value(!is_signed) } else { !is_min_value(!is_signed) });
    if !constant_is_ok {
        return None;
    }
    let new_pred = match pred {
        Ult => Ule,
        Ule => Ult,
        Ugt => Uge,
        Uge => Ugt,
        Slt => Sle,
        Sle => Slt,
        Sgt => Sge,
        Sge => Sgt,
        _ => unreachable!("Only for relational integer predicates."),
    };
    let new_bits = if will_increment { add(bits, 1, width) } else { sub(bits, 1, width) };
    Some((new_pred, same_sign, SelectOperand::Int { ty, bits: new_bits }))
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9230>
#[allow(clippy::too_many_arguments)]
fn match_select_pattern_pred(
    q: &Query,
    mut pred: Predicate,
    fmf: FastMathFlags,
    mut cmp_lhs: SelectOperand,
    mut cmp_rhs: SelectOperand,
    true_val: SelectOperand,
    false_val: SelectOperand,
    depth: u32,
    cast_op: Option<CastOp>,
) -> SelectPatternResult {
    if pred.is_fp() {
        let mut output_zero_val = None;
        if m_any_zero_fp(q, true_val) && !m_any_zero_fp(q, false_val) {
            output_zero_val = Some(true_val);
        } else if m_any_zero_fp(q, false_val) && !m_any_zero_fp(q, true_val) {
            output_zero_val = Some(false_val);
        }
        if let Some(zero) = output_zero_val {
            if m_any_zero_fp(q, cmp_lhs) && !same(q, cmp_lhs, zero) {
                cmp_lhs = zero;
            }
            if m_any_zero_fp(q, cmp_rhs) && !same(q, cmp_rhs, zero) {
                cmp_rhs = zero;
            }
        }
    }

    let mut lhs = Some(cmp_lhs);
    let mut rhs = Some(cmp_rhs);
    macro_rules! result {
        ($flavor:expr, $nan:expr, $ordered:expr) => {
            return SelectPatternResult { lhs, rhs, cast_op, ..SelectPatternResult::new($flavor, $nan, $ordered) }
        };
    }
    use SelectPatternFlavor as Spf;
    use SelectPatternNaNBehavior as Nan;

    if pred.is_fp() && !fmf.no_signed_zeros && !is_known_non_zero_fp(q, cmp_lhs) && !is_known_non_zero_fp(q, cmp_rhs) {
        result!(Spf::None, Nan::NA, false);
    }

    let mut nan_behavior = Nan::NA;
    let mut ordered = false;
    if pred.is_fp() {
        let lhs_safe = is_known_non_nan(q, cmp_lhs, fmf);
        let rhs_safe = is_known_non_nan(q, cmp_rhs, fmf);
        if lhs_safe && rhs_safe {
            nan_behavior = Nan::ReturnsAny;
            ordered = pred.is_ordered();
        } else if pred.is_ordered() {
            ordered = true;
            if lhs_safe {
                nan_behavior = Nan::ReturnsNaN;
            } else if rhs_safe {
                nan_behavior = Nan::ReturnsOther;
            } else {
                result!(Spf::None, Nan::NA, false);
            }
        } else {
            ordered = false;
            if lhs_safe {
                nan_behavior = Nan::ReturnsOther;
            } else if rhs_safe {
                nan_behavior = Nan::ReturnsNaN;
            } else {
                result!(Spf::None, Nan::NA, false);
            }
        }
    }

    if same(q, true_val, cmp_rhs) && same(q, false_val, cmp_lhs) {
        std::mem::swap(&mut cmp_lhs, &mut cmp_rhs);
        pred = pred.swapped();
        if nan_behavior == Nan::ReturnsNaN {
            nan_behavior = Nan::ReturnsOther;
        } else if nan_behavior == Nan::ReturnsOther {
            nan_behavior = Nan::ReturnsNaN;
        }
        ordered = !ordered;
    }

    if same(q, true_val, cmp_lhs) && same(q, false_val, cmp_rhs) {
        let found = get_select_pattern(pred, nan_behavior, ordered);
        result!(found.flavor, found.nan_behavior, found.ordered);
    }

    if is_known_negation(q, true_val, false_val, false, true) {
        let cmp_lhs_or_sext =
            |v: SelectOperand| same(q, v, cmp_lhs) || m_sext(q, v).is_some_and(|inner| same(q, inner, cmp_lhs));
        let maybe_sext_or_mul_cmp_lhs = |v: SelectOperand| {
            cmp_lhs_or_sext(v)
                || m_binary(q, v, BinaryOp::Mul).is_some_and(|(instruction, a, b)| {
                    instruction.flags.contains(Flags::NSW)
                        && cmp_lhs_or_sext(a)
                        && m_apint(q, b).is_some_and(|(bits, width)| bits != 0 && !is_negative(bits, width))
                })
                || m_binary(q, v, BinaryOp::Shl)
                    .is_some_and(|(instruction, a, _)| instruction.flags.contains(Flags::NSW) && cmp_lhs_or_sext(a))
        };
        let zero_or_all_ones = |v: SelectOperand| m_zero_int(q, v) || m_all_ones(q, v);
        let zero_or_one = |v: SelectOperand| m_zero_int(q, v) || m_one(q, v);
        let is_neg_of = |v: SelectOperand, of: SelectOperand| m_neg(q, v).is_some_and(|(_, inner)| same(q, inner, of));
        if maybe_sext_or_mul_cmp_lhs(true_val) {
            lhs = Some(true_val);
            rhs = Some(false_val);
            if is_neg_of(cmp_lhs, false_val) {
                std::mem::swap(&mut lhs, &mut rhs);
            }
            if pred == Predicate::Int(IntPredicate::Sgt) && zero_or_all_ones(cmp_rhs) {
                result!(Spf::Abs, Nan::NA, false);
            }
            if pred == Predicate::Int(IntPredicate::Sge) && zero_or_one(cmp_rhs) {
                result!(Spf::Abs, Nan::NA, false);
            }
            if pred == Predicate::Int(IntPredicate::Slt) && zero_or_one(cmp_rhs) {
                result!(Spf::NAbs, Nan::NA, false);
            }
        } else if maybe_sext_or_mul_cmp_lhs(false_val) {
            lhs = Some(false_val);
            rhs = Some(true_val);
            if is_neg_of(cmp_lhs, true_val) {
                std::mem::swap(&mut lhs, &mut rhs);
            }
            if pred == Predicate::Int(IntPredicate::Sgt) && zero_or_all_ones(cmp_rhs) {
                result!(Spf::NAbs, Nan::NA, false);
            }
            if pred == Predicate::Int(IntPredicate::Slt) && zero_or_one(cmp_rhs) {
                result!(Spf::Abs, Nan::NA, false);
            }
        }
    }

    if let Predicate::Int(int_pred) = pred {
        let found = match_min_max(q, int_pred, cmp_lhs, cmp_rhs, true_val, false_val, depth);
        return SelectPatternResult { cast_op, ..found };
    }

    if nan_behavior != Nan::ReturnsAny
        || (!fmf.no_signed_zeros && !is_known_non_zero_fp(q, cmp_lhs) && !is_known_non_zero_fp(q, cmp_rhs))
    {
        result!(Spf::None, Nan::NA, false);
    }
    let found = match_fast_float_clamp(q, pred, cmp_lhs, cmp_rhs, true_val, false_val);
    SelectPatternResult { cast_op, ..found }
}

/// `ConstantFoldCastOperand(op, c, dest)` of an integer, float or poison
/// constant.
fn fold_cast(
    q: &Query,
    op: CastOp,
    c: SelectOperand,
    dest: TypeId,
) -> Option<SelectOperand> {
    let types = &q.context.types;
    let c = canon(q, c);
    if let SelectOperand::Poison(_) = c {
        return Some(SelectOperand::Poison(dest));
    }
    let dest_float = || match types.get(dest) {
        Type::Float(kind) => Some(*kind),
        _ => None,
    };
    match (op, c) {
        (CastOp::Trunc | CastOp::ZExt, SelectOperand::Int { bits, .. }) => {
            Some(SelectOperand::Int { ty: dest, bits: bits & mask(types.int_bits(dest)?) })
        }
        (CastOp::SExt, SelectOperand::Int { ty, bits }) => {
            Some(SelectOperand::Int { ty: dest, bits: apint::sext(bits, types.int_bits(ty)?, types.int_bits(dest)?) })
        }
        (CastOp::FPTrunc | CastOp::FPExt, SelectOperand::Float { .. }) => {
            let value = fp_value(q, c)?;
            let bits = match dest_float()? {
                FloatKind::Float => u64::from((value as f32).to_bits()),
                _ => value.to_bits(),
            };
            Some(SelectOperand::Float { ty: dest, bits })
        }
        (CastOp::UIToFP | CastOp::SIToFP, SelectOperand::Int { ty, bits }) => {
            let kind = dest_float()?;
            let width = types.int_bits(ty)?;
            let bits = match (kind, op == CastOp::SIToFP) {
                (FloatKind::Float, true) => u64::from((apint::signed(bits, width) as f32).to_bits()),
                (FloatKind::Float, false) => u64::from((bits as f32).to_bits()),
                (_, true) => (apint::signed(bits, width) as f64).to_bits(),
                (_, false) => (bits as f64).to_bits(),
            };
            Some(SelectOperand::Float { ty: dest, bits })
        }
        (CastOp::FPToUI | CastOp::FPToSI, SelectOperand::Float { .. }) => {
            let width = types.int_bits(dest)?;
            let truncated = fp_value(q, c)?.trunc();
            let signed = op == CastOp::FPToSI;
            let half = 2.0_f64.powi(width as i32 - 1);
            let (low, high) = if signed { (-half, half) } else { (0.0, 2.0_f64.powi(width as i32)) };
            if truncated.is_nan() || truncated < low || truncated >= high {
                return Some(SelectOperand::Poison(dest));
            }
            let bits = if signed { (truncated as i128) as u128 & mask(width) } else { truncated as u128 };
            Some(SelectOperand::Int { ty: dest, bits })
        }
        _ => None,
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9394>
fn look_through_cast_const(
    q: &Query,
    cmp: &Instruction,
    src_ty: TypeId,
    c: Operand,
    cast_op: CastOp,
) -> Option<SelectOperand> {
    let cmp_pred = match cmp.opcode {
        Opcode::ICmp(pred) => Predicate::Int(pred),
        Opcode::FCmp(pred) => Predicate::Float(pred),
        _ => return None,
    };
    let casted_to = match cast_op {
        CastOp::ZExt if cmp_pred.is_unsigned() => fold_cast(q, CastOp::Trunc, c.into(), src_ty),
        CastOp::SExt if cmp_pred.is_signed() => fold_cast(q, CastOp::Trunc, c.into(), src_ty),
        CastOp::Trunc => {
            let cmp_const = cmp.operands[1];
            if matches!(cmp_const, Operand::Constant(_)) && q.type_of(cmp_const) == Some(src_ty) {
                Some(SelectOperand::Value(cmp_const))
            } else {
                let ext = if cmp_pred.is_signed() { CastOp::SExt } else { CastOp::ZExt };
                fold_cast(q, ext, c.into(), src_ty)
            }
        }
        CastOp::FPTrunc => fold_cast(q, CastOp::FPExt, c.into(), src_ty),
        CastOp::FPExt => fold_cast(q, CastOp::FPTrunc, c.into(), src_ty),
        CastOp::FPToUI => fold_cast(q, CastOp::UIToFP, c.into(), src_ty),
        CastOp::FPToSI => fold_cast(q, CastOp::SIToFP, c.into(), src_ty),
        CastOp::UIToFP => fold_cast(q, CastOp::FPToUI, c.into(), src_ty),
        CastOp::SIToFP => fold_cast(q, CastOp::FPToSI, c.into(), src_ty),
        _ => None,
    }?;
    let c_type = q.type_of(c)?;
    if let Some(casted_back) = fold_cast(q, cast_op, casted_to, c_type)
        && !same(q, casted_back, c.into())
    {
        return None;
    }
    Some(casted_to)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9489>
fn look_through_cast(
    q: &Query,
    cmp: &Instruction,
    v1: Operand,
    v2: Operand,
    cast_op: &mut Option<CastOp>,
) -> Option<SelectOperand> {
    let (_, cast1) = q.instruction_of(v1)?;
    let Opcode::Cast(op1) = cast1.opcode else { return None };
    *cast_op = Some(op1);
    let src_ty = q.type_of(cast1.operands[0])?;
    if let Some((_, cast2)) = q.instruction_of(v2)
        && let Opcode::Cast(op2) = cast2.opcode
    {
        if op1 == op2 && Some(src_ty) == q.type_of(cast2.operands[0]) {
            return Some(cast2.operands[0].into());
        }
        return None;
    }
    if matches!(v2, Operand::Constant(_)) {
        return look_through_cast_const(q, cmp, src_ty, v2, op1);
    }
    if op1 == CastOp::Trunc
        && let Some((_, ext)) = q.instruction_of(cmp.operands[1])
        && matches!(ext.opcode, Opcode::Cast(CastOp::ZExt | CastOp::SExt))
        && ext.operands[0] == v2
    {
        return Some(cmp.operands[1].into());
    }
    None
}

/// `matchSelectPattern(V, LHS, RHS)`: no `CastOp`, depth 0.
pub fn match_select_pattern(
    q: &Query,
    v: Operand,
) -> SelectPatternResult {
    match_select_pattern_cast(q, v.into(), false, 0)
}

/// `matchSelectPattern(V, LHS, RHS, CastOp, Depth)`; `want_cast` is a non-null
/// `CastOp`, and the result's `cast_op` is what it points to afterwards.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9530>
pub fn match_select_pattern_cast(
    q: &Query,
    v: SelectOperand,
    want_cast: bool,
    depth: u32,
) -> SelectPatternResult {
    if depth >= super::MAX_ANALYSIS_RECURSION_DEPTH {
        return SelectPatternResult::unknown();
    }
    let Some((_, select)) = instruction_of(q, v) else { return SelectPatternResult::unknown() };
    if select.opcode != Opcode::Select {
        return SelectPatternResult::unknown();
    }
    let Some((cmp, _)) = q.instruction_of(select.operands[0]) else { return SelectPatternResult::unknown() };
    let fmf = match q.context.types.get(select.ty) {
        Type::Float(_) => FastMathFlags {
            no_nans: select.flags.contains(Flags::NNAN),
            no_signed_zeros: select.flags.contains(Flags::NSZ),
        },
        _ => FastMathFlags::default(),
    };
    match_decomposed_select_pattern(q, cmp, select.operands[1], select.operands[2], fmf, want_cast, depth)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9550>
pub fn match_decomposed_select_pattern(
    q: &Query,
    cmp_inst: InstId,
    true_val: Operand,
    false_val: Operand,
    mut fmf: FastMathFlags,
    want_cast: bool,
    depth: u32,
) -> SelectPatternResult {
    let cmp = q.function.instruction(cmp_inst);
    let pred = match cmp.opcode {
        Opcode::ICmp(pred) => Predicate::Int(pred),
        Opcode::FCmp(pred) => {
            if cmp.flags.contains(Flags::NNAN) {
                fmf.no_nans = true;
            }
            Predicate::Float(pred)
        }
        _ => return SelectPatternResult::unknown(),
    };
    let (cmp_lhs, cmp_rhs) = (cmp.operands[0], cmp.operands[1]);
    if pred.is_equality() {
        return SelectPatternResult::unknown();
    }
    let mut cast_op = None;
    if want_cast && q.type_of(cmp_lhs) != q.type_of(true_val) {
        for (at, (first, second)) in [(true_val, false_val), (false_val, true_val)].into_iter().enumerate() {
            if let Some(c) = look_through_cast(q, cmp, first, second, &mut cast_op) {
                if matches!(cast_op, Some(CastOp::FPToSI | CastOp::FPToUI)) {
                    fmf.no_signed_zeros = true;
                }
                let (_, cast) = q.instruction_of(first).expect("a cast looked through");
                let source = SelectOperand::from(cast.operands[0]);
                let (t, f) = if at == 0 { (source, c) } else { (c, source) };
                return match_select_pattern_pred(q, pred, fmf, cmp_lhs.into(), cmp_rhs.into(), t, f, depth, cast_op);
            }
        }
    }
    match_select_pattern_pred(
        q,
        pred,
        fmf,
        cmp_lhs.into(),
        cmp_rhs.into(),
        true_val.into(),
        false_val.into(),
        depth,
        cast_op,
    )
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9588>
pub fn get_min_max_pred(
    spf: SelectPatternFlavor,
    ordered: bool,
) -> Predicate {
    use FloatPredicate as F;
    use IntPredicate as I;
    match spf {
        SelectPatternFlavor::SMin => Predicate::Int(I::Slt),
        SelectPatternFlavor::UMin => Predicate::Int(I::Ult),
        SelectPatternFlavor::SMax => Predicate::Int(I::Sgt),
        SelectPatternFlavor::UMax => Predicate::Int(I::Ugt),
        SelectPatternFlavor::FMinNum => Predicate::Float(if ordered { F::Olt } else { F::Ult }),
        SelectPatternFlavor::FMaxNum => Predicate::Float(if ordered { F::Ogt } else { F::Ugt }),
        _ => unreachable!("unhandled!"),
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9600>
pub fn get_min_max_intrinsic(spf: SelectPatternFlavor) -> IntrinsicId {
    match spf {
        SelectPatternFlavor::UMin => IntrinsicId::Umin,
        SelectPatternFlavor::UMax => IntrinsicId::Umax,
        SelectPatternFlavor::SMin => IntrinsicId::Smin,
        SelectPatternFlavor::SMax => IntrinsicId::Smax,
        _ => unreachable!("Unexpected SPF"),
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9615>
pub fn get_inverse_min_max_flavor(spf: SelectPatternFlavor) -> SelectPatternFlavor {
    match spf {
        SelectPatternFlavor::SMin => SelectPatternFlavor::SMax,
        SelectPatternFlavor::UMin => SelectPatternFlavor::UMax,
        SelectPatternFlavor::SMax => SelectPatternFlavor::SMin,
        SelectPatternFlavor::UMax => SelectPatternFlavor::UMin,
        _ => unreachable!("unhandled!"),
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9623>
pub fn get_inverse_min_max_intrinsic(min_max_id: IntrinsicId) -> IntrinsicId {
    match min_max_id {
        IntrinsicId::Smax => IntrinsicId::Smin,
        IntrinsicId::Smin => IntrinsicId::Smax,
        IntrinsicId::Umax => IntrinsicId::Umin,
        IntrinsicId::Umin => IntrinsicId::Umax,
        IntrinsicId::Maximum => IntrinsicId::Minimum,
        IntrinsicId::Minimum => IntrinsicId::Maximum,
        IntrinsicId::Maxnum => IntrinsicId::Minnum,
        IntrinsicId::Minnum => IntrinsicId::Maxnum,
        IntrinsicId::Maximumnum => IntrinsicId::Minimumnum,
        IntrinsicId::Minimumnum => IntrinsicId::Maximumnum,
        _ => unreachable!("Unexpected intrinsic"),
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9643>
pub fn get_min_max_limit(
    spf: SelectPatternFlavor,
    bit_width: u32,
) -> u128 {
    match spf {
        SelectPatternFlavor::SMax => smax(bit_width),
        SelectPatternFlavor::SMin => smin(bit_width),
        SelectPatternFlavor::UMax => mask(bit_width),
        SelectPatternFlavor::UMin => 0,
        _ => unreachable!("Unexpected flavor"),
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9653>
pub fn can_convert_to_min_or_max_intrinsic(
    q: &Query,
    vl: &[Operand],
) -> (Option<IntrinsicId>, bool) {
    let mut all_cmp_single_use = true;
    let mut select_pattern = SelectPatternFlavor::None;
    let all = vl
        .iter()
        .all(
            |&i| {
                let current = match_select_pattern(q, i);
                if !current.flavor.is_min_or_max() {
                    return false;
                }
                if select_pattern != SelectPatternFlavor::None && select_pattern != current.flavor {
                    return false;
                }
                select_pattern = current.flavor;
                let one_use = q
                    .instruction_of(i)
                    .is_some_and(
                        |(_, select)| select.opcode == Opcode::Select
                            && matches!(
                                select.operands[0],
                                Operand::Value(cond) if q.function.users(cond).only().is_some()
                            ),
                    );
                all_cmp_single_use &= one_use;
                true
            },
        );
    if all {
        let id = match select_pattern {
            SelectPatternFlavor::SMin => IntrinsicId::Smin,
            SelectPatternFlavor::UMin => IntrinsicId::Umin,
            SelectPatternFlavor::SMax => IntrinsicId::Smax,
            SelectPatternFlavor::UMax => IntrinsicId::Umax,
            SelectPatternFlavor::FMaxNum => IntrinsicId::Maxnum,
            SelectPatternFlavor::FMinNum => IntrinsicId::Minnum,
            _ => unreachable!("unexpected select pattern flavor"),
        };
        return (Some(id), all_cmp_single_use);
    }
    (None, false)
}

// ---------------------------------------------------------------------------
// computeConstantRange
// ---------------------------------------------------------------------------

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L10344>
fn set_limits_for_bin_op(
    q: &Query,
    bo: &Instruction,
    op: BinaryOp,
    width: u32,
    prefer_signed_range: bool,
) -> (u128, u128) {
    let (mut lower, mut upper) = (0_u128, 0_u128);
    let apint_at = |at: usize| m_apint(q, bo.operands[at].into()).map(|(bits, _)| bits);
    let has_nsw = bo.flags.contains(Flags::NSW);
    let has_nuw = bo.flags.contains(Flags::NUW);
    let exact = bo.flags.contains(Flags::EXACT);
    let (smin_v, smax_v) = (smin(width), smax(width));
    match op {
        BinaryOp::Sub => {
            if let Some(c) = apint_at(0) {
                let has_nuw = has_nuw && !(prefer_signed_range && has_nsw);
                if has_nuw {
                    upper = add(c, 1, width);
                } else if has_nsw {
                    if is_negative(c, width) {
                        lower = smin_v;
                        upper = sub(c, smax_v, width);
                    } else {
                        lower = sub(c, smax_v, width);
                        upper = smin_v;
                    }
                }
            }
        }
        BinaryOp::Add => {
            if let Some(c) = apint_at(1).filter(|c| *c != 0) {
                let has_nuw = has_nuw && !(prefer_signed_range && has_nsw);
                if has_nuw {
                    lower = c;
                } else if has_nsw {
                    if is_negative(c, width) {
                        lower = smin_v;
                        upper = add(add(smax_v, c, width), 1, width);
                    } else {
                        lower = add(smin_v, c, width);
                        upper = add(smax_v, 1, width);
                    }
                }
            }
        }
        BinaryOp::And => {
            if let Some(c) = apint_at(1) {
                upper = add(c, 1, width);
            }
            let neg_of = |a: Operand, b: Operand| m_neg(q, a.into()).is_some_and(|(_, inner)| same(q, inner, b.into()));
            if neg_of(bo.operands[0], bo.operands[1]) || neg_of(bo.operands[1], bo.operands[0]) {
                upper = add(smin_v, 1, width);
            }
        }
        BinaryOp::Or => {
            if let Some(c) = apint_at(1) {
                lower = c;
            }
        }
        BinaryOp::AShr => {
            if let Some(c) = apint_at(1).filter(|c| *c < u128::from(width)) {
                lower = apint::ashr(smin_v, width, c as u32);
                upper = add(apint::ashr(smax_v, width, c as u32), 1, width);
            } else if let Some(c) = apint_at(0) {
                let mut shift_amount = width - 1;
                if c != 0 && exact {
                    shift_amount = apint::countr_zero(c, width);
                }
                if is_negative(c, width) {
                    lower = c;
                    upper = add(apint::ashr(c, width, shift_amount), 1, width);
                } else {
                    lower = apint::ashr(c, width, shift_amount);
                    upper = add(c, 1, width);
                }
            }
        }
        BinaryOp::LShr => {
            if let Some(c) = apint_at(1).filter(|c| *c < u128::from(width)) {
                upper = add(apint::lshr(mask(width), c as u32), 1, width);
            } else if let Some(c) = apint_at(0) {
                let mut shift_amount = width - 1;
                if c != 0 && exact {
                    shift_amount = apint::countr_zero(c, width);
                }
                lower = apint::lshr(c, shift_amount);
                upper = add(c, 1, width);
            }
        }
        BinaryOp::Shl => {
            if let Some(c) = apint_at(0) {
                if has_nuw {
                    lower = c;
                    upper = add(apint::shl(lower, width, apint::countl_zero(lower, width)), 1, width);
                } else if has_nsw {
                    if is_negative(c, width) {
                        let shift_amount = apint::countl_one(c, width) - 1;
                        lower = apint::shl(c, width, shift_amount);
                        upper = add(c, 1, width);
                    } else {
                        let shift_amount = apint::countl_zero(c, width) - 1;
                        lower = c;
                        upper = add(apint::shl(c, width, shift_amount), 1, width);
                    }
                } else {
                    if apint::bit(c, 0) {
                        lower = 1;
                    }
                    upper = add(apint::set_high(0, width, apint::popcount(c)), 1, width);
                }
            } else if let Some(c) = apint_at(1).filter(|c| *c < u128::from(width)) {
                upper = add(apint::set_range(0, c as u32, width), 1, width);
            }
        }
        BinaryOp::SDiv => {
            if let Some(c) = apint_at(1) {
                if c == mask(width) {
                    lower = add(smin_v, 1, width);
                    upper = add(smax_v, 1, width);
                } else if apint::countl_zero(c, width) < width - 1 {
                    lower = apint::sdiv(smin_v, c, width);
                    upper = apint::sdiv(smax_v, c, width);
                    if apint::signed(lower, width) > apint::signed(upper, width) {
                        std::mem::swap(&mut lower, &mut upper);
                    }
                    upper = add(upper, 1, width);
                }
            } else if let Some(c) = apint_at(0) {
                if c == smin_v {
                    lower = c;
                    upper = add(apint::lshr(lower, 1), 1, width);
                } else {
                    upper = add(abs(c, width), 1, width);
                    lower = add(sub(0, upper, width), 1, width);
                }
            }
        }
        BinaryOp::UDiv => {
            if let Some(c) = apint_at(1).filter(|c| *c != 0) {
                upper = add(mask(width) / c, 1, width);
            } else if let Some(c) = apint_at(0) {
                upper = add(c, 1, width);
            }
        }
        BinaryOp::SRem => {
            if let Some(c) = apint_at(1) {
                upper = abs(c, width);
                lower = add(sub(0, upper, width), 1, width);
            } else if let Some(c) = apint_at(0) {
                if is_negative(c, width) {
                    upper = 1;
                    lower = c;
                } else {
                    upper = add(c, 1, width);
                }
            }
        }
        BinaryOp::URem => {
            if let Some(c) = apint_at(1) {
                upper = c;
            } else if let Some(c) = apint_at(0) {
                upper = add(c, 1, width);
            }
        }
        _ => {}
    }
    (lower, upper)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L10569>
fn get_range_for_intrinsic(
    q: &Query,
    id: IntrinsicId,
    arguments: &[Operand],
    width: u32,
) -> ConstantRange {
    let arg = |at: usize| arguments.get(at).copied();
    let c_of = |at: usize| arg(at).and_then(|one| m_apint(q, one.into())).map(|(bits, _)| bits);
    let c_either = || c_of(0).or_else(|| c_of(1));
    let non_empty = |lower: u128, upper: u128| ConstantRange::non_empty(width, lower, upper);
    let (smin_v, smax_v) = (smin(width), smax(width));
    match id {
        IntrinsicId::Ctlz | IntrinsicId::Cttz => {
            let mut upper = u128::from(width) & mask(width);
            if !arg(1).is_some_and(|one| m_one(q, one.into())) {
                upper = add(upper, 1, width);
            }
            return non_empty(0, upper);
        }
        IntrinsicId::Ctpop => return non_empty(0, add(u128::from(width), 1, width)),
        IntrinsicId::UaddSat => {
            if let Some(c) = c_either() {
                return non_empty(c, 0);
            }
        }
        IntrinsicId::SaddSat => {
            if let Some(c) = c_either() {
                if is_negative(c, width) {
                    return non_empty(smin_v, add(add(smax_v, c, width), 1, width));
                }
                return non_empty(add(smin_v, c, width), add(smax_v, 1, width));
            }
        }
        IntrinsicId::UsubSat => {
            if let Some(c) = c_of(0) {
                return non_empty(0, add(c, 1, width));
            }
            if let Some(c) = c_of(1) {
                return non_empty(0, add(sub(mask(width), c, width), 1, width));
            }
        }
        IntrinsicId::SsubSat => {
            if let Some(c) = c_of(0) {
                if is_negative(c, width) {
                    return non_empty(smin_v, add(sub(c, smin_v, width), 1, width));
                }
                return non_empty(sub(c, smax_v, width), add(smax_v, 1, width));
            } else if let Some(c) = c_of(1) {
                if is_negative(c, width) {
                    return non_empty(sub(smin_v, c, width), add(smax_v, 1, width));
                }
                return non_empty(smin_v, add(sub(smax_v, c, width), 1, width));
            }
        }
        IntrinsicId::Umin | IntrinsicId::Umax | IntrinsicId::Smin | IntrinsicId::Smax => {
            if let Some(c) = c_either() {
                return match id {
                    IntrinsicId::Umin => non_empty(0, add(c, 1, width)),
                    IntrinsicId::Umax => non_empty(c, 0),
                    IntrinsicId::Smin => non_empty(smin_v, add(c, 1, width)),
                    _ => non_empty(c, add(smax_v, 1, width)),
                };
            }
        }
        IntrinsicId::Abs => {
            if arg(1).is_some_and(|one| m_one(q, one.into())) {
                return non_empty(0, add(smax_v, 1, width));
            }
            return non_empty(0, add(smin_v, 1, width));
        }
        // No `vscale_range` attribute exists in MIR: `getVScaleRange` says the
        // vscale is non-zero.
        IntrinsicId::Vscale => return ConstantRange::new(width, 1, 0),
        _ => {}
    }
    ConstantRange::full(width)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L10691>
fn get_range_for_select_pattern(
    q: &Query,
    si: Operand,
    bit_width: u32,
) -> ConstantRange {
    let r = match_select_pattern(q, si);
    if r.flavor == SelectPatternFlavor::None {
        return ConstantRange::full(bit_width);
    }
    let (Some(lhs), Some(rhs)) = (r.lhs, r.rhs) else { return ConstantRange::full(bit_width) };
    let non_empty = |lower: u128, upper: u128| ConstantRange::non_empty(bit_width, lower, upper);
    let (smin_v, smax_v) = (smin(bit_width), smax(bit_width));
    if r.flavor == SelectPatternFlavor::Abs {
        let negated_with_nsw = m_neg(q, rhs)
            .is_some_and(|(instruction, inner)| same(q, inner, lhs) && instruction.flags.contains(Flags::NSW));
        if negated_with_nsw {
            return non_empty(0, add(smax_v, 1, bit_width));
        }
        return non_empty(0, add(smin_v, 1, bit_width));
    }
    if r.flavor == SelectPatternFlavor::NAbs {
        return non_empty(smin_v, 1);
    }
    let Some((c, _)) = m_apint(q, lhs).or_else(|| m_apint(q, rhs)) else { return ConstantRange::full(bit_width) };
    match r.flavor {
        SelectPatternFlavor::UMin => non_empty(0, add(c, 1, bit_width)),
        SelectPatternFlavor::UMax => non_empty(c, 0),
        SelectPatternFlavor::SMin => non_empty(smin_v, add(c, 1, bit_width)),
        SelectPatternFlavor::SMax => non_empty(c, add(smax_v, 1, bit_width)),
        _ => ConstantRange::full(bit_width),
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L10738>
fn set_limit_for_fp_to_i() -> (u128, u128) {
    // `half` is not a MIR type: the source is never one, and nothing is set.
    (0, 0)
}

/// `CallBase::getRange`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instructions.cpp#L381>
fn call_base_range(
    q: &Query,
    inst: &Instruction,
    width: u32,
) -> Option<ConstantRange> {
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &inst.opcode else { return None };
    let range_of = |attrs: &[Attribute]| {
        attrs
            .iter()
            .find_map(
                |attr| match attr {
                    Attribute::Range { lower, upper, .. } => Some(ConstantRange::new(width, *lower, *upper)),
                    _ => None,
                },
            )
    };
    let call_attr = range_of(&info.return_attrs);
    let from_end = if matches!(inst.opcode, Opcode::Invoke(_)) { 3 } else { 1 };
    let callee = inst.operands.len().checked_sub(from_end).and_then(|at| inst.operands.get(at));
    let fn_attr = match callee {
        Some(Operand::Constant(id)) => match q.context.get(*id).kind {
            ConstantKind::Global(global) => match &q.globals.get(global.0 as usize)?.kind {
                GlobalKind::Function(function) => range_of(&function.return_attrs),
                GlobalKind::Variable(_) => None,
            },
            _ => None,
        },
        _ => None,
    };
    match (call_attr, fn_attr) {
        (Some(a), Some(b)) => Some(a.intersect_with(&b, PreferredRangeType::Smallest)),
        (Some(a), None) | (None, Some(a)) => Some(a),
        (None, None) => None,
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L10755>
pub fn compute_constant_range(
    q: &Query,
    v: Operand,
    for_signed: bool,
    depth: u32,
) -> ConstantRange {
    let bit_width = q.scalar_bits(v);
    if depth == super::MAX_ANALYSIS_RECURSION_DEPTH {
        return ConstantRange::full(bit_width);
    }
    if let Operand::Constant(_) = v {
        return match q.int_constant(v) {
            Some((bits, width)) => ConstantRange::single(width, bits),
            None => ConstantRange::full(bit_width),
        };
    }

    let mut cr = ConstantRange::full(bit_width);
    let found = q.instruction_of(v);
    if let Some((inst, instruction)) = found {
        let intrinsic = match instruction.opcode {
            Opcode::Call(_) => intrinsic_id(q, inst),
            _ => None,
        };
        match (&instruction.opcode, intrinsic) {
            (Opcode::Binary(op), _) => {
                let (lower, upper) = set_limits_for_bin_op(q, instruction, *op, bit_width, for_signed);
                cr = ConstantRange::non_empty(bit_width, lower, upper);
            }
            (_, Some(id)) => {
                let arguments = instruction.operands.split_last().map_or(&[][..], |(_, arguments)| arguments);
                cr = get_range_for_intrinsic(q, id, arguments, bit_width);
            }
            (Opcode::Select, _) => {
                let cr_true = compute_constant_range(q, instruction.operands[1], for_signed, depth + 1);
                let cr_false = compute_constant_range(q, instruction.operands[2], for_signed, depth + 1);
                cr = cr_true.union_with(&cr_false, PreferredRangeType::Smallest);
                cr = cr.intersect_with(&get_range_for_select_pattern(q, v, bit_width), PreferredRangeType::Smallest);
            }
            (Opcode::Cast(CastOp::Trunc), _) => {
                let src_cr = compute_constant_range(q, instruction.operands[0], for_signed, depth + 1);
                cr = src_cr.truncate(bit_width, 0);
            }
            (Opcode::Cast(CastOp::FPToUI | CastOp::FPToSI), _) => {
                let (lower, upper) = set_limit_for_fp_to_i();
                cr = ConstantRange::non_empty(bit_width, lower, upper);
            }
            _ => {}
        }
    } else if let Operand::Value(value) = v
        && let ValueDef::Argument(index) = q.function.value(value).def
        && let Some(attrs) = q.function.parameter_attrs.get(index as usize)
        && let Some((lower, upper)) = attrs.iter().find_map(|attr| match attr {
            Attribute::Range { lower, upper, .. } => Some((*lower, *upper)),
            _ => None,
        })
    {
        cr = ConstantRange::new(bit_width, lower, upper);
    }

    if let Some((_, instruction)) = found {
        if let Some((_, node)) = instruction.metadata.iter().find(|(kind, _)| kind == "range") {
            let operands = q.metadata.get(node.0 as usize).map(|node| node.operands.as_slice()).unwrap_or_default();
            let bound = |at: usize| match operands.get(at) {
                Some(MetadataOperand::Constant(id)) => match q.context.get(*id).kind {
                    ConstantKind::Int(bits) => Some(bits),
                    _ => None,
                },
                _ => None,
            };
            let pairs: Vec<(u128, u128)> =
                (0..operands.len() / 2).filter_map(|one| Some((bound(2 * one)?, bound(2 * one + 1)?))).collect();
            if !pairs.is_empty() {
                let from_metadata = ConstantRange::from_range_pairs(bit_width, &pairs);
                cr = cr.intersect_with(&from_metadata, PreferredRangeType::Smallest);
            }
        }
        if let Some(range) = call_base_range(q, instruction, bit_width) {
            cr = cr.intersect_with(&range, PreferredRangeType::Smallest);
        }
    }

    if let (Some(cxt), Some(assumptions)) = (q.cxt, q.assumptions) {
        for &assume in &assumptions.assumes {
            let call = q.function.instruction(assume);
            let Some((_, cmp)) = call.operands.first().and_then(|arg| q.instruction_of(*arg)) else { continue };
            let Opcode::ICmp(pred) = cmp.opcode else { continue };
            if cmp.operands[0] != v || !is_valid_assume_for_context(q, assume, cxt, false) {
                continue;
            }
            let rhs = compute_constant_range(&q.with_cxt(Some(assume)), cmp.operands[1], false, depth + 1);
            let same_sign = cmp.flags.contains(Flags::SAMESIGN);
            cr = cr.intersect_with(
                &ConstantRange::make_allowed_icmp_region_samesign(pred, same_sign, &rhs),
                PreferredRangeType::Smallest,
            );
        }
    }
    cr
}

/// The `cru` and `crs` columns of `examples/vtdump.rs`.
pub fn column(
    q: &Query,
    v: Operand,
    for_signed: bool,
) -> String {
    if q.is_pointer(v) {
        return String::new();
    }
    format!(" {}={}", if for_signed { "crs" } else { "cru" }, compute_constant_range(q, v, for_signed, 0).print())
}

/// One value of the `pat` column: a name, or the constant's type and bits.
fn pattern_value(
    q: &Query,
    v: SelectOperand,
) -> String {
    match canon(q, v) {
        SelectOperand::Int { ty, bits } => format!("i{}:{bits}", q.context.types.int_bits(ty).unwrap_or(0)),
        SelectOperand::Float { bits, .. } => format!("f:{bits:X}"),
        SelectOperand::Poison(_) => "poison".to_owned(),
        SelectOperand::Value(Operand::Value(id)) => q.function.value(id).name.clone().unwrap_or_else(|| "?".to_owned()),
        SelectOperand::Value(_) => "?".to_owned(),
    }
}

/// The `sp` column of `examples/vtdump.rs`: `matchSelectPattern` of a select as
/// `flavor/NaN behaviour/ordered/cast/LHS/RHS`, `-` of anything else.
pub fn pattern_column(
    q: &Query,
    v: Operand,
) -> String {
    let is_select = q.instruction_of(v).is_some_and(|(_, instruction)| instruction.opcode == Opcode::Select);
    if !is_select {
        return " pat=-".to_owned();
    }
    let r = match_select_pattern_cast(q, v.into(), true, 0);
    let flavor = match r.flavor {
        SelectPatternFlavor::None => return " pat=none".to_owned(),
        SelectPatternFlavor::SMin => "smin",
        SelectPatternFlavor::UMin => "umin",
        SelectPatternFlavor::SMax => "smax",
        SelectPatternFlavor::UMax => "umax",
        SelectPatternFlavor::FMinNum => "fminnum",
        SelectPatternFlavor::FMaxNum => "fmaxnum",
        SelectPatternFlavor::Abs => "abs",
        SelectPatternFlavor::NAbs => "nabs",
    };
    let nan = match r.nan_behavior {
        SelectPatternNaNBehavior::NA => "na",
        SelectPatternNaNBehavior::ReturnsNaN => "nan",
        SelectPatternNaNBehavior::ReturnsOther => "other",
        SelectPatternNaNBehavior::ReturnsAny => "any",
    };
    let cast =
        r.cast_op.and_then(|op| crate::opcode::CAST.iter().find(|(one, _)| *one == op)).map_or("-", |(_, name)| *name);
    let value = |one: Option<SelectOperand>| one.map_or("?".to_owned(), |one| pattern_value(q, one));
    format!(" pat={flavor}/{nan}/{}/{cast}/{}/{}", u8::from(r.ordered), value(r.lhs), value(r.rhs))
}

fn overflow_letter(overflow: OverflowResult) -> char {
    match overflow {
        OverflowResult::MayOverflow => 'M',
        OverflowResult::AlwaysOverflowsLow => 'L',
        OverflowResult::AlwaysOverflowsHigh => 'H',
        OverflowResult::NeverOverflows => 'N',
    }
}

/// The `ov` column of `examples/vtdump.rs`: `computeOverflowFor*` of an add
/// (unsigned, signed of the add, signed of its operands), a sub (unsigned,
/// signed) or a mul (unsigned, signed), as letters; `-` of anything else.
pub fn overflow_column(
    q: &Query,
    v: Operand,
) -> String {
    let Some((inst, instruction)) = q.instruction_of(v) else { return " ov=-".to_owned() };
    let Opcode::Binary(op) = instruction.opcode else { return " ov=-".to_owned() };
    let (a, b) = (instruction.operands[0], instruction.operands[1]);
    let letters: Vec<OverflowResult> = match op {
        BinaryOp::Add => vec![
            compute_overflow_for_unsigned_add(q, a, b),
            compute_overflow_for_signed_add_operator(q, inst),
            compute_overflow_for_signed_add(q, a, b),
        ],
        BinaryOp::Sub => vec![compute_overflow_for_unsigned_sub(q, a, b), compute_overflow_for_signed_sub(q, a, b)],
        BinaryOp::Mul => vec![
            compute_overflow_for_unsigned_mul(q, a, b, instruction.flags.contains(Flags::NSW)),
            compute_overflow_for_signed_mul(q, a, b),
        ],
        _ => return " ov=-".to_owned(),
    };
    format!(" ov={}", letters.into_iter().map(overflow_letter).collect::<String>())
}
