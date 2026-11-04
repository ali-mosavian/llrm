//! `computeKnownBits`: which bits of a value are proven zero and which one.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ValueTracking.cpp`
//! (`computeKnownBits`, `computeKnownBitsFromOperator` and the helpers they
//! call), one function per C++ function, each linking the line it translates.
//! Depth stops at [`super::MAX_ANALYSIS_RECURSION_DEPTH`]. The `m_*` matchers
//! are closures over [`super::pattern`], so a binding made by a failed
//! alternative survives into the next, as in C++.
//!
//! Not translated, as MIR has no such thing:
//! - vectors and scalable vectors: `getShuffleDemandedElts` (L127-L140),
//!   `computeKnownBitsForHorizontalOperation` (L1257-L1281), the
//!   `ShuffleVector`, `InsertElement` and `ExtractElement` cases (L2398-L2480),
//!   the vector `BitCast` cases (L1596-L1660), constant vectors (L2599-L2641),
//!   the `vector_*` and `stepvector` intrinsics (L2211-L2247, L2370-L2397), the
//!   x86 `pmulh*` and `ph*` intrinsics (L2280-L2293, L2297-L2330), `ptrtoaddr`;
//! - floating point classes: `FPToSI` (L1528-L1540) and the float to integer
//!   `BitCast` (L1584-L1595), both `computeKnownFPClass`;
//! - `getAbsoluteSymbolRange` of a global (L2672-L2675), `isReadVLENB` and
//!   `getRISCVVLENBRange` (L1319-L1350: `read_register` takes metadata);
//! - the `vscale_range` attribute, which MIR does not parse: `getVScaleRange`
//!   (L1297-L1316) is the range without it;
//! - `smulh` and `umulh` above 64 bits, where `KnownBits::mulhs` stops.
//!
//! Approximated by what MIR holds: a global without `align`, an `alloca`
//! without `align` and an `sret` parameter have their type's ABI alignment,
//! where LLVM has the preferred one, which MIR's `DataLayout` lacks
//! (`Value::getPointerAlignment`, `DataLayout::getPreferredAlign`); a
//! function's pointer has alignment 1.
//!
//! # Example
//! `and i32 %x, 255` has its top 24 bits known zero.

use std::cell::Cell;

use super::MAX_ANALYSIS_RECURSION_DEPTH;
use super::context::{
    break_self_recursive_phi, compute_known_bits_from_cond, compute_known_bits_from_context,
    is_implied_by_dom_condition,
};
use super::guarantee::{is_guaranteed_not_to_be_poison, is_guaranteed_not_to_be_undef};
use super::nonzero::is_known_non_zero;
use super::pattern::nested::{
    any, bind, bind_apint, cast, exactly, match_any_binary, match_binary, match_icmp, neg, not, specific, specific_int,
};
use super::pattern::{
    IntrinsicId, all_ones, apint, argument_range, call_arguments, call_intrinsic as intrinsic_id, call_range,
    call_return_align, is_intrinsic, match_simple_recurrence, match_two_input_recurrence, one, phi_incoming, result_of,
    returned_argument_operand, zero,
};
use super::query::Query;
use super::range::{
    SelectPatternFlavor, get_inverse_min_max_flavor, m_apint, match_select_pattern, match_select_pattern_cast,
};
use super::signbits::compute_num_sign_bits;
use crate::apint::{countl_zero, countr_zero, set_high, set_range, sext, sign_bit, signed, umul_ov};
use crate::constantrange::ConstantRange;
use crate::context::{ConstantExpr, ConstantKind, mask};
use crate::knownbits::KnownBits;
use crate::module::{
    GlobalKind, GlobalValue, InstId, Instruction, Linkage, MetadataNode, MetadataOperand, Operand, ValueDef,
};
use crate::opcode::{Attribute, BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use crate::types::{Type, TypeId};

/// `getBitWidth(Ty, DL)` of a value.
fn width_of(
    q: &Query,
    v: Operand,
) -> u32 {
    q.scalar_bits(v).max(1)
}

/// `safeCxtI(V, CxtI)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L113>
pub fn safe_cxt_i(
    q: &Query,
    v: Operand,
    cxt: Option<InstId>,
) -> Option<InstId> {
    if let Some(cxt) = cxt.filter(|cxt| q.block_of(*cxt).is_some()) {
        return Some(cxt);
    }
    q.instruction_of(v).map(|(inst, _)| inst).filter(|inst| q.block_of(*inst).is_some())
}

/// The `KnownBits`-returning `computeKnownBits(V, DL, AC, CxtI, DT,
/// UseInstrInfo, Depth)`: `cxt` goes through `safeCxtI`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L166>
pub fn compute_known_bits_at(
    q: &Query,
    v: Operand,
    cxt: Option<InstId>,
    depth: u32,
) -> KnownBits {
    compute_known_bits(&q.with_cxt(safe_cxt_i(q, v, cxt)), v, depth)
}

/// `NoCommonBitsSetResult`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoCommonBitsSetResult {
    Unknown,
    OnlyIfUndefIgnored,
    Known,
}

/// `haveNoCommonBitsSetSpecialCases(LHS, RHS, SQ)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L175>
fn have_no_common_bits_set_special_cases(
    q: &Query,
    lhs: Operand,
    rhs: Operand,
) -> NoCommonBitsSetResult {
    use NoCommonBitsSetResult::{Known, OnlyIfUndefIgnored, Unknown};
    let no_undef = |v: Operand| is_guaranteed_not_to_be_undef(q, v, 0);
    let result = |known: bool| if known { Known } else { OnlyIfUndefIgnored };
    let none = Flags::default();
    let (and, xor) = (BinaryOp::And, BinaryOp::Xor);

    // Look for an inverted mask: (X & ~M) op (Y & M).
    {
        let m = Cell::new(None);
        if match_binary(q, lhs, and, none, true, &|x| not(q, x, &bind(&m)), &any)
            && match_binary(q, rhs, and, none, true, &specific(&m), &any)
        {
            return result(no_undef(m.get().expect("bound")));
        }
    }

    // X op (Y & ~X)
    if match_binary(q, rhs, and, none, true, &|x| not(q, x, &exactly(lhs)), &any) {
        return result(no_undef(lhs));
    }

    // X op ((X & Y) ^ Y)
    let y = Cell::new(None);
    if match_binary(
        q,
        rhs,
        xor,
        none,
        true,
        &|x| match_binary(q, x, and, none, true, &exactly(lhs), &bind(&y)),
        &specific(&y),
    ) {
        return result(no_undef(lhs) && no_undef(y.get().expect("bound")));
    }

    // (ext Y) op ext(~Y)
    let ext = |v: Operand, inner: &dyn Fn(Operand) -> bool| {
        cast(q, v, CastOp::ZExt, inner) || cast(q, v, CastOp::SExt, inner)
    };
    if ext(lhs, &bind(&y)) && ext(rhs, &|x| not(q, x, &specific(&y))) {
        return result(no_undef(y.get().expect("bound")));
    }

    // (A & B) op ~(A | B)
    {
        let (a, b) = (Cell::new(None), Cell::new(None));
        if match_binary(q, lhs, and, none, false, &bind(&a), &bind(&b))
            && not(q, rhs, &|x| match_binary(q, x, BinaryOp::Or, none, true, &specific(&a), &specific(&b)))
        {
            return result(no_undef(a.get().expect("bound")) && no_undef(b.get().expect("bound")));
        }
    }

    // (X << V) op (Y >> (BitWidth - V)) or (X >> V) op (Y << (BitWidth - V))
    {
        let (v, r) = (Cell::new(None), Cell::new(None));
        let shifted = |value: Operand, op: BinaryOp, amount: &dyn Fn(Operand) -> bool| {
            match_binary(q, value, op, none, false, &any, amount)
        };
        let sub = |x: Operand| match_binary(q, x, BinaryOp::Sub, none, false, &bind_apint(q, &r), &bind(&v));
        let in_either_order = (shifted(rhs, BinaryOp::Shl, &sub) && shifted(lhs, BinaryOp::LShr, &specific(&v)))
            || (shifted(rhs, BinaryOp::LShr, &sub) && shifted(lhs, BinaryOp::Shl, &specific(&v)));
        if in_either_order && r.get().is_some_and(|(bits, _)| bits >= u128::from(width_of(q, lhs))) {
            return Known;
        }
    }

    Unknown
}

/// `getNoCommonBitsSetResult(LHSCache, RHSCache, SQ)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L241>
pub fn get_no_common_bits_set_result(
    q: &Query,
    lhs: Operand,
    rhs: Operand,
) -> NoCommonBitsSetResult {
    use NoCommonBitsSetResult::{Known, OnlyIfUndefIgnored, Unknown};
    let result = have_no_common_bits_set_special_cases(q, lhs, rhs);
    if result == Known {
        return Known;
    }
    let commute_result = have_no_common_bits_set_special_cases(q, rhs, lhs);
    if commute_result == Known {
        return Known;
    }
    if KnownBits::have_no_common_bits_set(&compute_known_bits(q, lhs, 0), &compute_known_bits(q, rhs, 0)) {
        return Known;
    }
    if result == OnlyIfUndefIgnored || commute_result == OnlyIfUndefIgnored {
        return OnlyIfUndefIgnored;
    }
    Unknown
}

/// `haveNoCommonBitsSet(LHSCache, RHSCache, SQ)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L272>
pub fn have_no_common_bits_set(
    q: &Query,
    lhs: Operand,
    rhs: Operand,
) -> bool {
    get_no_common_bits_set_result(q, lhs, rhs) == NoCommonBitsSetResult::Known
}

/// Whether `inst` has users and each is an `icmp` of it with zero whose
/// predicate `accept` takes.
fn only_used_in_zero_comparison(
    q: &Query,
    inst: InstId,
    accept: &dyn Fn(IntPredicate) -> bool,
) -> bool {
    let Some(result) = q.function.instruction(inst).result else { return false };
    let users = q.function.users(result);
    !users.is_empty()
        && users.iter().all(|used| {
            let user = q.function.instruction(used.user);
            matches!(user.opcode, Opcode::ICmp(pred) if accept(pred)) && zero(q, user.operands[1])
        })
}

/// `isOnlyUsedInZeroComparison(I)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L280>
pub fn is_only_used_in_zero_comparison(
    q: &Query,
    inst: InstId,
) -> bool {
    only_used_in_zero_comparison(q, inst, &|_| true)
}

/// `isOnlyUsedInZeroEqualityComparison(I)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L285>
pub fn is_only_used_in_zero_equality_comparison(
    q: &Query,
    inst: InstId,
) -> bool {
    only_used_in_zero_comparison(q, inst, &|pred| matches!(pred, IntPredicate::Eq | IntPredicate::Ne))
}

/// `isKnownNonNegative(V, SQ, Depth)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L305>
pub fn is_known_non_negative(
    q: &Query,
    v: Operand,
    depth: u32,
) -> bool {
    compute_known_bits(q, v, depth).is_non_negative()
}

/// `isKnownPositive(V, SQ, Depth)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L310>
pub fn is_known_positive(
    q: &Query,
    v: Operand,
    depth: u32,
) -> bool {
    if let Some((bits, width)) = q.int_constant(v) {
        return bits != 0 && bits & sign_bit::<u128>(width) == 0;
    }
    let known = compute_known_bits(q, v, depth);
    known.is_non_negative() && (known.is_non_zero() || is_known_non_zero(q, v, depth))
}

/// `isKnownNegative(V, SQ, Depth)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L322>
pub fn is_known_negative(
    q: &Query,
    v: Operand,
    depth: u32,
) -> bool {
    compute_known_bits(q, v, depth).is_negative()
}

/// `MaskedValueIsZero(V, Mask, SQ, Depth)`: `mask` has the width of `v`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L345>
pub fn masked_value_is_zero(
    q: &Query,
    v: Operand,
    mask: u128,
    depth: u32,
) -> bool {
    mask & !compute_known_bits(q, v, depth).zero == 0
}

/// `computeKnownBitsFromLerpPattern(Op0, Op1, DemandedElts, KnownOut, Q,
/// Depth)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L399>
fn compute_known_bits_from_lerp_pattern(
    q: &Query,
    op0: Operand,
    op1: Operand,
    known_out: &mut KnownBits,
    depth: u32,
) {
    let bit_width = width_of(q, op0);
    let none = Flags::default();
    let (a, b, c, d) = (Cell::new(None), Cell::new(None), Cell::new(None), Cell::new(None));
    let sub_bc: Cell<Option<InstId>> = Cell::new(None);

    let match_sub_bc = |v: Operand| {
        let found = match_binary(q, v, BinaryOp::Sub, Flags::NUW, false, &bind(&b), &bind(&c))
            || match_binary(q, v, BinaryOp::Xor, none, false, &bind(&c), &bind(&b));
        if found {
            sub_bc.set(q.instruction_of(v).map(|(inst, _)| inst));
        }
        found
    };
    let match_a_sub_bc =
        |v: Operand| match_binary(q, v, BinaryOp::Mul, none, true, &bind(&a), &match_sub_bc) || match_sub_bc(v);
    let match_cd =
        |v: Operand| match_binary(q, v, BinaryOp::Mul, none, true, &bind(&d), &specific(&c)) || specific(&c)(v);
    let matches = |lhs: Operand, rhs: Operand| match_a_sub_bc(lhs) && match_cd(rhs);

    if !matches(op0, op1) && !matches(op1, op0) {
        return;
    }

    let known_or_one = |v: Option<Operand>| match v {
        Some(v) => compute_known_bits(q, v, depth + 1),
        None => KnownBits::constant(bit_width, 1),
    };

    let known_a = known_or_one(a.get());
    if !known_a.is_non_negative() {
        return;
    }
    let known_d = known_or_one(d.get());
    if !known_d.is_non_negative() {
        return;
    }
    let known_b = compute_known_bits(q, b.get().expect("matched"), depth + 1);
    if !known_b.is_non_negative() {
        return;
    }
    let known_c = compute_known_bits(q, c.get().expect("matched"), depth + 1);
    if !known_c.is_non_negative() {
        return;
    }

    let is_xor =
        sub_bc.get().is_some_and(|inst| matches!(
            q.function.instruction(inst).opcode,
            Opcode::Binary(BinaryOp::Xor)
        ));
    if is_xor && known_c.max_value() & !known_b.min_value() != 0 {
        return;
    }

    let max_ad = known_a.max_value().max(known_d.max_value());
    let (upper_bound, overflow) = umul_ov(max_ad, known_b.max_value(), bit_width);
    if overflow {
        return;
    }
    known_out.zero = set_high(known_out.zero, bit_width, countl_zero(upper_bound, bit_width));
}

/// `m_Intrinsic<ID>(L, R)` and, `commutable`, `m_c_Intrinsic<ID>(L, R)`.
fn match_intrinsic(
    q: &Query,
    v: Operand,
    id: IntrinsicId,
    commutable: bool,
    l: &dyn Fn(Operand) -> bool,
    r: &dyn Fn(Operand) -> bool,
) -> bool {
    let Some((inst, instruction)) = q.instruction_of(v) else { return false };
    if intrinsic_id(q, inst) != Some(id) {
        return false;
    }
    let arguments = call_arguments(instruction);
    let (Some(&a), Some(&b)) = (arguments.first(), arguments.get(1)) else { return false };
    (l(a) && r(b)) || (commutable && l(b) && r(a))
}

/// `computeKnownBitsAddSub(Add, Op0, Op1, NSW, NUW, DemandedElts, KnownOut,
/// Known2, Q, Depth)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L513>
fn compute_known_bits_add_sub(
    q: &Query,
    add: bool,
    op0: Operand,
    op1: Operand,
    nsw: bool,
    nuw: bool,
    depth: u32,
) -> KnownBits {
    let mut known_out = compute_known_bits(q, op1, depth + 1);

    if known_out.is_unknown() && !nsw && !nuw {
        return known_out;
    }

    let known2 = compute_known_bits(q, op0, depth + 1);
    known_out = KnownBits::compute_for_add_sub(add, nsw, nuw, &known2, &known_out);

    if !add
        && nsw
        && !known_out.is_non_negative()
        && (q.cxt.and_then(|cxt| is_implied_by_dom_condition(q, IntPredicate::Sle, op1, op0, cxt)).unwrap_or(false)
            || match_intrinsic(q, op1, IntrinsicId::Smin, true, &exactly(op0), &any))
    {
        known_out.make_non_negative();
    }

    if add {
        compute_known_bits_from_lerp_pattern(q, op0, op1, &mut known_out, depth);
    }
    known_out
}

/// `computeKnownBitsMul(Op0, Op1, NSW, NUW, DemandedElts, Known, Known2, Q,
/// Depth)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L539>
fn compute_known_bits_mul(
    q: &Query,
    op0: Operand,
    op1: Operand,
    nsw: bool,
    nuw: bool,
    depth: u32,
) -> KnownBits {
    let mut known = compute_known_bits(q, op1, depth + 1);
    let known2 = compute_known_bits(q, op0, depth + 1);

    let mut is_known_negative = false;
    let mut is_known_non_negative = false;
    if nsw {
        if op0 == op1 {
            is_known_non_negative = true;
        } else {
            let is_known_non_negative_op1 = known.is_non_negative();
            let is_known_non_negative_op0 = known2.is_non_negative();
            let is_known_negative_op1 = known.is_negative();
            let is_known_negative_op0 = known2.is_negative();
            is_known_non_negative = (is_known_negative_op1 && is_known_negative_op0)
                || (is_known_non_negative_op1 && is_known_non_negative_op0);
            if !is_known_non_negative && nuw {
                let one = KnownBits::constant(known.width, 1);
                is_known_non_negative =
                    KnownBits::sgt(&known, &one).unwrap_or(false) || KnownBits::sgt(&known2, &one).unwrap_or(false);
            }
            if !is_known_non_negative {
                is_known_negative = (is_known_negative_op1 && is_known_non_negative_op0 && known2.is_non_zero())
                    || (is_known_negative_op0 && is_known_non_negative_op1 && known.is_non_zero());
            }
        }
    }

    let mut self_multiply = op0 == op1;
    if self_multiply {
        self_multiply &= is_guaranteed_not_to_be_undef(q, op0, depth + 1);
    }
    known = KnownBits::mul(&known, &known2, self_multiply);

    if self_multiply {
        let sign_bits = compute_num_sign_bits(q, op0, depth + 1);
        let ty_bits = width_of(q, op0);
        let out_valid_bits = 2 * (ty_bits - sign_bits + 1);
        if out_valid_bits < ty_bits {
            known.zero = set_high(known.zero, ty_bits, ty_bits - out_valid_bits + 1);
        }
    }

    if is_known_non_negative && !known.is_negative() {
        known.make_non_negative();
    } else if is_known_negative && !known.is_non_negative() {
        known.make_negative();
    }
    known
}

/// `computeKnownBitsFromRangeMetadata(Ranges, Known)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L607>
pub fn compute_known_bits_from_range_metadata(
    q: &Query,
    ranges: &MetadataNode,
    known: &mut KnownBits,
) {
    let bit_width = known.width;
    let num_ranges = ranges.operands.len() / 2;
    known.set_all_conflict();
    let bound = |at: usize| match ranges.operands.get(at) {
        Some(MetadataOperand::Constant(id)) => q.int_constant(Operand::Constant(*id)),
        _ => None,
    };
    for i in 0..num_ranges {
        let (Some((lower, width)), Some((upper, _))) = (bound(2 * i), bound(2 * i + 1)) else { continue };
        let range = ConstantRange::new(width, lower, upper);
        let common_prefix_bits = countl_zero(range.get_unsigned_max() ^ range.get_unsigned_min(), width);
        let prefix = set_high(0, bit_width, common_prefix_bits);
        let unsigned_max = range.get_unsigned_max() & mask(bit_width);
        known.one &= unsigned_max & prefix;
        known.zero &= !unsigned_max & prefix;
    }
}

/// `Q.IIQ.getMetadata(I, MD_range)`.
fn range_metadata<'a>(
    q: &Query<'a>,
    instruction: &Instruction,
) -> Option<&'a MetadataNode> {
    let (_, node) = instruction.metadata.iter().find(|(kind, _)| kind == "range")?;
    q.metadata.get(node.0 as usize)
}

/// `computeKnownBitsFromShiftOperator(I, DemandedElts, Known, Known2, Q, Depth,
/// KF)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L1171>
fn compute_known_bits_from_shift_operator(
    q: &Query,
    instruction: &Instruction,
    depth: u32,
    kf: &dyn Fn(&KnownBits, &KnownBits, bool) -> KnownBits,
) -> KnownBits {
    let known2 = compute_known_bits(q, instruction.operands[0], depth + 1);
    let known = compute_known_bits(q, instruction.operands[1], depth + 1);
    let sh_amt_non_zero = known.is_non_zero()
        || (known.max_value() < u128::from(known.width) && is_known_non_zero(q, instruction.operands[1], depth + 1));
    kf(&known2, &known, sh_amt_non_zero)
}

/// `getKnownBitsFromAndXorOr(I, DemandedElts, KnownLHS, KnownRHS, Q, Depth)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L1187>
fn get_known_bits_from_and_xor_or(
    q: &Query,
    instruction: &Instruction,
    known_lhs: &KnownBits,
    known_rhs: &KnownBits,
    depth: u32,
) -> KnownBits {
    let this = result_of(instruction);
    let none = Flags::default();
    let mut is_and = false;
    let has_known_one = known_lhs.one != 0 || known_rhs.one != 0;
    let x: Cell<Option<Operand>> = Cell::new(None);
    let y: Cell<Option<Operand>> = Cell::new(None);

    let mut known_out;
    match instruction.opcode {
        Opcode::Binary(BinaryOp::And) => {
            known_out = known_lhs.and(known_rhs);
            is_and = true;
            if has_known_one
                && match_binary(q, this, BinaryOp::And, none, true, &bind(&x), &|n| neg(q, n, &specific(&x)))
            {
                if known_lhs.count_max_trailing_zeros() <= known_rhs.count_max_trailing_zeros() {
                    known_out = known_lhs.blsi();
                } else {
                    known_out = known_rhs.blsi();
                }
            }
        }
        Opcode::Binary(BinaryOp::Or) => {
            known_out = known_lhs.or(known_rhs);
        }
        Opcode::Binary(BinaryOp::Xor) => {
            known_out = known_lhs.xor(known_rhs);
            if has_known_one
                && match_binary(q, this, BinaryOp::Xor, none, true, &bind(&x), &|n| {
                    match_binary(q, n, BinaryOp::Add, none, false, &specific(&x), &|c| all_ones(q, c))
                })
            {
                let x_bits = if Some(instruction.operands[0]) == x.get() { known_lhs } else { known_rhs };
                known_out = x_bits.blsmsk();
            }
        }
        _ => unreachable!("Invalid Op used in 'analyzeKnownBitsFromAndXorOr'"),
    }

    if known_out.zero & 1 == 0
        && known_out.one & 1 == 0
        && (match_any_binary(q, this, true, &bind(&x), &|n| {
            match_binary(q, n, BinaryOp::Add, none, true, &specific(&x), &bind(&y))
        })
        .is_some()
            || match_any_binary(q, this, true, &bind(&x), &|n| {
                match_binary(q, n, BinaryOp::Sub, none, false, &specific(&x), &bind(&y))
            })
            .is_some()
            || match_any_binary(q, this, true, &bind(&x), &|n| {
                match_binary(q, n, BinaryOp::Sub, none, false, &bind(&y), &specific(&x))
            })
            .is_some())
    {
        let known_y = compute_known_bits(q, y.get().expect("matched"), depth + 1);
        if known_y.count_min_trailing_ones() > 0 {
            if is_and {
                known_out.zero |= 1;
            } else {
                known_out.one |= 1;
            }
        }
    }
    known_out
}

/// `analyzeKnownBitsFromAndXorOr(I, KnownLHS, KnownRHS, SQ, Depth)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L1284>
pub fn analyze_known_bits_from_and_xor_or(
    q: &Query,
    inst: InstId,
    known_lhs: &KnownBits,
    known_rhs: &KnownBits,
    depth: u32,
) -> KnownBits {
    get_known_bits_from_and_xor_or(q, q.function.instruction(inst), known_lhs, known_rhs, depth)
}

/// `getVScaleRange(F, BitWidth)`: MIR has no `vscale_range`, so the range
/// without it.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L1297>
pub fn get_vscale_range(bit_width: u32) -> ConstantRange {
    ConstantRange::new(bit_width, 1, 0)
}

/// `adjustKnownBitsForSelectArm(Known, Cond, Arm, Invert, Q, Depth)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L1352>
pub fn adjust_known_bits_for_select_arm(
    q: &Query,
    known: &mut KnownBits,
    cond: Operand,
    arm: Operand,
    invert: bool,
    depth: u32,
) {
    if known.is_constant() {
        return;
    }

    let mut cond_res = KnownBits::unknown(known.width);
    compute_known_bits_from_cond(q, arm, cond, &mut cond_res, invert, depth + 1);
    if cond_res.is_unknown() {
        return;
    }

    cond_res = cond_res.union_with(known);
    if cond_res.has_conflict() {
        return;
    }

    if !is_guaranteed_not_to_be_undef(q, arm, depth + 1) {
        return;
    }

    *known = cond_res;
}

/// `isSignedMinMaxClamp(Select, In, CLow, CHigh)`: the clamped value and the
/// bounds.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L1388>
pub fn is_signed_min_max_clamp(
    q: &Query,
    select: Operand,
) -> Option<(Operand, u128, u128)> {
    let result = match_select_pattern(q, select);
    let spf = result.flavor;
    if spf != SelectPatternFlavor::SMax && spf != SelectPatternFlavor::SMin {
        return None;
    }

    let (mut c_low, width) = m_apint(q, result.rhs?)?;

    let result2 = match_select_pattern_cast(q, result.lhs?, false, 0);
    if get_inverse_min_max_flavor(spf) != result2.flavor {
        return None;
    }

    let mut c_high = m_apint(q, result2.rhs?)?.0;

    if spf == SelectPatternFlavor::SMin {
        std::mem::swap(&mut c_low, &mut c_high);
    }

    let input = result2.lhs?.operand()?;
    (signed(c_low, width) <= signed(c_high, width)).then_some((input, c_low, c_high))
}

/// `isSignedMinMaxIntrinsicClamp(II, CLow, CHigh)`: the bounds.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L1417>
pub fn is_signed_min_max_intrinsic_clamp(
    q: &Query,
    inst: InstId,
) -> Option<(u128, u128)> {
    let id = intrinsic_id(q, inst)?;
    let inverse = match id {
        IntrinsicId::Smin => IntrinsicId::Smax,
        IntrinsicId::Smax => IntrinsicId::Smin,
        _ => unreachable!("Must be smin/smax"),
    };
    let arguments = call_arguments(q.function.instruction(inst));
    let (inner, inner_instruction) = q.instruction_of(arguments[0])?;
    if intrinsic_id(q, inner) != Some(inverse) {
        return None;
    }
    let (mut c_low, width) = apint(q, arguments[1])?;
    let mut c_high = apint(q, call_arguments(inner_instruction)[1])?.0;

    if id == IntrinsicId::Smin {
        std::mem::swap(&mut c_low, &mut c_high);
    }
    (signed(c_low, width) <= signed(c_high, width)).then_some((c_low, c_high))
}

/// `unionWithMinMaxIntrinsicClamp(II, Known)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L1436>
fn union_with_min_max_intrinsic_clamp(
    q: &Query,
    inst: InstId,
    known: &mut KnownBits,
) {
    if let Some((c_low, c_high)) = is_signed_min_max_intrinsic_clamp(q, inst) {
        let width = known.width;
        *known = known
            .union_with(&ConstantRange::non_empty(width, c_low, c_high.wrapping_add(1) & mask(width)).to_known_bits());
    }
}

/// `computeKnownBitsForRecurrenceOperands(P, Start, Step, DemandedElts,
/// KnownStart, KnownStep, Q, Depth)`: the known bits of start and of step.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L1444>
fn compute_known_bits_for_recurrence_operands(
    q: &Query,
    phi: InstId,
    start: Operand,
    step: Operand,
    depth: u32,
) -> (KnownBits, KnownBits) {
    let incoming: Vec<_> = phi_incoming(q.function.instruction(phi)).collect();
    let op_num = if incoming[0].0 == start { 0 } else { 1 };

    let rec_q = q.without_cond_context().with_cxt(q.function.terminator(incoming[op_num].1));
    let known_start = compute_known_bits(&rec_q, start, depth + 1);

    let rec_q = q.without_cond_context().with_cxt(q.function.terminator(incoming[1 - op_num].1));
    let known_step = compute_known_bits(&rec_q, step, depth + 1);
    (known_start, known_step)
}

/// `isa_and_nonnull<UndefValue>(P->hasConstantValue())`.
fn phi_has_undef_constant_value(
    q: &Query,
    phi: &Instruction,
) -> bool {
    let this = result_of(phi);
    let incoming: Vec<Operand> = phi_incoming(phi).map(|(value, _)| value).collect();
    let mut constant_value = incoming[0];
    for &value in &incoming[1..] {
        if value != constant_value && value != this {
            if constant_value != this {
                return false;
            }
            constant_value = value;
        }
    }
    if constant_value == this {
        return true;
    }
    matches!(
        constant_value,
        Operand::Constant(id) if matches!(q.context().get(id).kind, ConstantKind::Poison | ConstantKind::Undef)
    )
}

/// `Type::isSized()`.
pub(crate) fn is_sized(
    q: &Query,
    ty: TypeId,
) -> bool {
    let types = &q.context().types;
    match types.get(ty) {
        Type::Int(_) | Type::Float(_) | Type::Pointer(_) => true,
        Type::Vector { element, .. } | Type::Array { element, .. } => is_sized(q, *element),
        Type::Struct { fields, .. } => fields.iter().all(|field| is_sized(q, *field)),
        Type::Named(_) => types.fields(ty).is_some_and(|fields| fields.iter().all(|field| is_sized(q, *field))),
        Type::Void | Type::Label | Type::Metadata | Type::Token | Type::Function { .. } => false,
    }
}

/// Where a `gep_type_iterator` is: the element type a sequential index steps
/// over, a struct, or a vector.
#[derive(Clone, Copy)]
enum GepType {
    Element(TypeId),
    Struct(TypeId),
    Vector(TypeId),
    Done,
}

/// The `GetElementPtr` case of `computeKnownBitsFromOperator`.
fn known_bits_of_get_element_ptr(
    q: &Query,
    instruction: &Instruction,
    source: TypeId,
    known: &mut KnownBits,
    depth: u32,
) {
    let types = &q.context().types;
    let bit_width = known.width;
    *known = compute_known_bits(q, instruction.operands[0], depth + 1);
    let Type::Pointer(space) = *types.get(instruction.ty) else { return };
    let index_width = q.layout.pointer(space).index_bits;
    let mut acc_const_indices: u128 = 0;

    let add_index_to_known = |known: &mut KnownBits, index_bits: &KnownBits| {
        if index_width == bit_width {
            *known = KnownBits::add(known, index_bits, false, false, false);
        } else {
            let low = KnownBits::add(&known.trunc(index_width), index_bits, false, false, false);
            known.insert_bits(&low, 0);
        }
    };

    let mut gti = GepType::Element(source);
    for &index in &instruction.operands[1..] {
        if known.is_unknown() {
            break;
        }

        let indexed = |gti: GepType| match gti {
            GepType::Element(ty) => Some(ty),
            GepType::Vector(vector) => match types.get(vector) {
                Type::Vector { element, .. } => Some(*element),
                _ => None,
            },
            GepType::Struct(s) => {
                let (bits, _) = q.int_constant(index)?;
                types.member(s, u64::try_from(bits).ok()?)
            }
            GepType::Done => None,
        };
        let step = |gti: GepType| match indexed(gti).map(|ty| (ty, types.get(ty))) {
            Some((_, Type::Array { element, .. })) => GepType::Element(*element),
            Some((ty, Type::Vector { .. })) => GepType::Vector(ty),
            Some((ty, Type::Struct { .. } | Type::Named(_))) => GepType::Struct(ty),
            _ => GepType::Done,
        };

        if zero(q, index) {
            gti = step(gti);
            continue;
        }

        if let GepType::Struct(s) = gti {
            let Some((idx, _)) = q.int_constant(index) else { return };
            let (_, offsets) = q.layout.struct_layout(types, s);
            acc_const_indices = acc_const_indices.wrapping_add(u128::from(offsets[idx as usize])) & mask(index_width);
            gti = step(gti);
            continue;
        }

        let Some(indexed_ty) = indexed(gti) else { return };
        if !is_sized(q, indexed_ty) {
            known.reset_all();
            break;
        }

        let stride_in_bytes = match gti {
            GepType::Vector(_) => q.layout.store_size(types, indexed_ty),
            _ => q.layout.alloc_size(types, indexed_ty),
        };
        if let Some((bits, width)) = q.int_constant(index) {
            let index_value =
                if width < index_width { sext(bits, width, index_width) } else { bits & mask(index_width) };
            acc_const_indices = acc_const_indices.wrapping_add(index_value.wrapping_mul(u128::from(stride_in_bytes)))
                & mask(index_width);
            gti = step(gti);
            continue;
        }

        let index_bits = compute_known_bits(q, index, depth + 1).sext_or_trunc(index_width);
        let scaling_factor = KnownBits::constant(index_width, u128::from(stride_in_bytes) & mask(index_width));
        add_index_to_known(known, &KnownBits::mul(&index_bits, &scaling_factor, false));
        gti = step(gti);
    }
    if !known.is_unknown() && acc_const_indices != 0 {
        add_index_to_known(known, &KnownBits::constant(index_width, acc_const_indices));
    }
}

/// The `PHI` case of `computeKnownBitsFromOperator`.
fn known_bits_of_phi(
    q: &Query,
    phi: InstId,
    instruction: &Instruction,
    known: &mut KnownBits,
    depth: u32,
) {
    let bit_width = known.width;
    let this = result_of(instruction);

    if let Some((bo, start, step)) = match_simple_recurrence(q, phi) {
        let bo_instruction = q.function.instruction(bo);
        let Opcode::Binary(opcode) = bo_instruction.opcode else { unreachable!("a binary operator") };

        match opcode {
            BinaryOp::LShr | BinaryOp::AShr | BinaryOp::Shl | BinaryOp::UDiv | BinaryOp::URem
                if opcode == BinaryOp::URem || bo_instruction.operands[0] == this =>
            {
                let rec_q = q.without_cond_context().with_cxt(Some(phi));
                let known_start = compute_known_bits(&rec_q, start, depth + 1);
                match opcode {
                    BinaryOp::Shl => {
                        known.zero |= mask(known_start.count_min_trailing_zeros());
                    }
                    BinaryOp::LShr | BinaryOp::UDiv | BinaryOp::URem => {
                        known.zero = set_high(known.zero, bit_width, known_start.count_min_leading_zeros());
                    }
                    _ => {
                        known.zero = set_high(known.zero, bit_width, known_start.count_min_leading_zeros());
                        known.one = set_high(known.one, bit_width, known_start.count_min_leading_ones());
                    }
                }
            }

            BinaryOp::Add | BinaryOp::Sub | BinaryOp::And | BinaryOp::Or | BinaryOp::Mul => {
                let (known_start, known_step) = compute_known_bits_for_recurrence_operands(q, phi, start, step, depth);

                known.zero |= mask(known_start.count_min_trailing_zeros().min(known_step.count_min_trailing_zeros()));

                let overflowing = matches!(opcode, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul);
                if overflowing && bo_instruction.flags.contains(Flags::NSW) {
                    match opcode {
                        BinaryOp::Add => {
                            if known_start.is_non_negative() && known_step.is_non_negative() {
                                known.make_non_negative();
                            } else if known_start.is_negative() && known_step.is_negative() {
                                known.make_negative();
                            }
                        }
                        BinaryOp::Sub => {
                            if bo_instruction.operands[0] == this {
                                if known_start.is_non_negative() && known_step.is_negative() {
                                    known.make_non_negative();
                                } else if known_start.is_negative() && known_step.is_non_negative() {
                                    known.make_negative();
                                }
                            }
                        }
                        _ => {
                            if known_start.is_non_negative() && known_step.is_non_negative() {
                                known.make_non_negative();
                            }
                        }
                    }
                }
            }

            _ => {}
        }
    } else if let Some((ii, start, step)) = match_two_input_recurrence(q, phi, &|inst, _| is_intrinsic(q, inst)) {
        let id = intrinsic_id(q, ii);
        if id == Some(IntrinsicId::Umin) || id == Some(IntrinsicId::Umax) {
            let (known_start, known_step) = compute_known_bits_for_recurrence_operands(q, phi, start, step, depth);

            if id == Some(IntrinsicId::Umin) {
                known.zero = set_high(known.zero, bit_width, known_start.count_min_leading_zeros());
                known.one = set_high(
                    known.one,
                    bit_width,
                    known_start.count_min_leading_ones().min(known_step.count_min_leading_ones()),
                );
            } else {
                known.zero = set_high(
                    known.zero,
                    bit_width,
                    known_start.count_min_leading_zeros().min(known_step.count_min_leading_zeros()),
                );
                known.one = set_high(known.one, bit_width, known_start.count_min_leading_ones());
            }
        }
    }

    if instruction.operands.is_empty() {
        return;
    }

    if depth < MAX_ANALYSIS_RECURSION_DEPTH - 1 && known.is_unknown() {
        if phi_has_undef_constant_value(q, instruction) {
            return;
        }

        known.set_all_conflict();
        for slot in 0..instruction.operands.len() / 2 {
            let mut cxt_i = None;
            let mut cxt_phi = phi;
            let inc_value = break_self_recursive_phi(q, phi, 2 * slot, &mut cxt_i, Some(&mut cxt_phi));
            if inc_value == this {
                continue;
            }

            let rec_q = q.without_cond_context().with_cxt(cxt_i);

            let mut known2 = compute_known_bits(&rec_q, inc_value, MAX_ANALYSIS_RECURSION_DEPTH - 1);

            if !known2.is_constant() {
                let branch =
                    cxt_i.map(|inst| q.function.instruction(inst)).filter(|branch| branch.opcode == Opcode::Br);
                if let Some(branch) = branch.filter(|branch| branch.operands.len() == 3) {
                    let rhs = Cell::new(None);
                    let pred = match_icmp(q, branch.operands[0], true, &exactly(inc_value), &bind_apint(q, &rhs));
                    if let (Some(pred), Operand::Block(true_succ), Operand::Block(false_succ)) =
                        (pred, branch.operands[1], branch.operands[2])
                    {
                        let phi_block = q.block_of(cxt_phi);
                        if (Some(true_succ) == phi_block) != (Some(false_succ) == phi_block) {
                            let pred = if Some(false_succ) == phi_block { pred.inverse() } else { pred };
                            let (rhs, width) = rhs.get().expect("matched");
                            let cr = ConstantRange::make_exact_icmp_region(pred, width, rhs);
                            let known_union = known2.union_with(&cr.to_known_bits());
                            if known_union.has_conflict() {
                                known.reset_all();
                                break;
                            }
                            known2 = known_union;
                        }
                    }
                }
            }

            *known = known.intersect_with(&known2);
            if known.is_unknown() {
                break;
            }
        }
    }
}

/// `Value::getPointerAlignment(DL)`.
pub fn pointer_alignment(
    q: &Query,
    v: Operand,
) -> u64 {
    let types = &q.context().types;
    let abi = |ty: TypeId| if is_sized(q, ty) { Some(q.layout.align(types, ty)) } else { None };
    let align_attribute = |attrs: &[Attribute]| {
        attrs
            .iter()
            .find_map(
                |attr| match attr {
                    Attribute::Int(name, value) if name == "align" => Some(*value),
                    _ => None,
                },
            )
    };
    match v {
        Operand::Constant(id) => match &q.context().get(id).kind {
            ConstantKind::Global(global) => match q.globals.get(global.0 as usize) {
                Some(GlobalValue { kind: GlobalKind::Variable(variable), linkage, .. }) => {
                    if let Some(align) = variable.align {
                        return align;
                    }
                    let Some(align) = abi(variable.ty) else { return 1 };
                    let strong = variable.initializer.is_some()
                        && !matches!(
                            linkage,
                            Linkage::Weak
                                | Linkage::WeakOdr
                                | Linkage::LinkOnce
                                | Linkage::LinkOnceOdr
                                | Linkage::Common
                                | Linkage::ExternWeak
                                | Linkage::AvailableExternally
                        );
                    if !strong {
                        return align;
                    }
                    // `getPreferredAlign` places a global at the type's
                    // preferred alignment. llrm's backend aligns a global only
                    // as stated (`variable.align`, data.rs), so the ABI one is
                    // all a pointer to it can be taken to have; the 16 of a
                    // large global is the old rule, kept.
                    if align < 16 && q.layout.size_bits(types, variable.ty) > 128 { 16 } else { align }
                }
                _ => 1,
            },
            ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::IntToPtr, value }) => {
                match q.int_constant(Operand::Constant(*value)) {
                    Some((bits, width)) => {
                        let trailing_zeros = countr_zero(bits, width);
                        if trailing_zeros < 32 { 1_u64 << trailing_zeros } else { 1_u64 << 32 }
                    }
                    None => 1,
                }
            }
            _ => 1,
        },
        Operand::Value(id) => match q.function.value(id).def {
            ValueDef::Argument(index) => {
                let attrs = q.function.parameter_attrs.get(index as usize).map_or(&[][..], Vec::as_slice);
                if let Some(align) = align_attribute(attrs) {
                    return align;
                }
                attrs
                    .iter()
                    .find_map(|attr| match attr {
                        Attribute::Type(name, ty) if name == "sret" => abi(*ty),
                        _ => None,
                    })
                    .unwrap_or(1)
            }
            ValueDef::Instruction(inst) => {
                let instruction = q.function.instruction(inst);
                match &instruction.opcode {
                    Opcode::Alloca { allocated, align, .. } => {
                        align.unwrap_or_else(|| q.layout.align(types, *allocated))
                    }
                    Opcode::Call(_) | Opcode::Invoke(_) => call_return_align(q, instruction).unwrap_or(1),
                    Opcode::Load { .. } => instruction
                        .metadata
                        .iter()
                        .find(|(kind, _)| kind == "align")
                        .and_then(|(_, node)| q.metadata.get(node.0 as usize))
                        .and_then(|node| match node.operands.first() {
                            Some(MetadataOperand::Constant(id)) => q.int_constant(Operand::Constant(*id)),
                            _ => None,
                        })
                        .map_or(1, |(bits, _)| u64::try_from(bits).unwrap_or(u64::MAX).max(1)),
                    _ => 1,
                }
            }
        },
        Operand::Block(_) => 1,
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Value.cpp#L918
/// `Value::getPointerDereferenceableBytes(DL, CanBeNull)`: the bytes known
/// dereferenceable at the pointer `v`, and whether it can be null.
pub fn pointer_dereferenceable_bytes(
    q: &Query,
    v: Operand,
) -> (u64, bool) {
    let types = &q.context().types;
    let int_attr = |attrs: &[Attribute], wanted: &str| {
        attrs
            .iter()
            .find_map(|attr| match attr {
                Attribute::Int(name, value) if name == wanted => Some(*value),
                _ => None,
            })
            .unwrap_or(0)
    };
    // `getMetadata(MD_dereferenceable...)` of a load or `inttoptr`: the first
    // operand of the node.
    let metadata_bytes = |inst: InstId, kind: &str| {
        q.metadata_node(inst, kind)
            .and_then(|node| match node.operands.first() {
                Some(MetadataOperand::Constant(id)) => q.int_constant(Operand::Constant(*id)),
                _ => None,
            })
            .map_or(0, |(bits, _)| u64::try_from(bits).unwrap_or(u64::MAX))
    };

    let mut deref_bytes = 0;
    let mut can_be_null = false;
    match v {
        Operand::Value(id) => match q.function.value(id).def {
            ValueDef::Argument(index) => {
                let attrs = q.function.parameter_attrs.get(index as usize).map_or(&[][..], Vec::as_slice);
                deref_bytes = int_attr(attrs, "dereferenceable");
                if deref_bytes == 0 {
                    // Handle byval/byref/inalloca/preallocated arguments
                    let pointee = attrs
                        .iter()
                        .find_map(
                            |attr| match attr {
                                Attribute::Type(name, ty)
                                    if matches!(name.as_str(), "byval" | "byref" | "inalloca" | "preallocated") =>
                                {
                                    Some(*ty)
                                }
                                _ => None,
                            },
                        );
                    if let Some(ty) = pointee.filter(|ty| is_sized(q, *ty)) {
                        // FIXME: Why isn't this the type alloc size?
                        deref_bytes = q.layout.store_size(types, ty);
                    }
                }
                if deref_bytes == 0 {
                    deref_bytes = int_attr(attrs, "dereferenceable_or_null");
                    can_be_null = true;
                }
            }
            ValueDef::Instruction(inst) => {
                let instruction = q.function.instruction(inst);
                match &instruction.opcode {
                    Opcode::Call(info) | Opcode::Invoke(info) => {
                        let callee = super::pattern::called_function(q, instruction);
                        let ret = |name: &str| {
                            int_attr(&info.return_attrs, name)
                                .max(callee.map_or(0, |function| int_attr(&function.return_attrs, name)))
                        };
                        deref_bytes = ret("dereferenceable");
                        if deref_bytes == 0 {
                            deref_bytes = ret("dereferenceable_or_null");
                            can_be_null = true;
                        }
                    }
                    Opcode::Load { .. } | Opcode::Cast(CastOp::IntToPtr) => {
                        deref_bytes = metadata_bytes(inst, "dereferenceable");
                        if deref_bytes == 0 {
                            deref_bytes = metadata_bytes(inst, "dereferenceable_or_null");
                            can_be_null = true;
                        }
                    }
                    Opcode::Alloca { allocated, .. } => {
                        // `getAllocationSize`: the allocated type's size, times
                        // a constant array size.
                        if is_sized(q, *allocated) {
                            let count = match instruction.operands.first() {
                                None => Some(1),
                                Some(count) => q.int_constant(*count).and_then(|(bits, _)| u64::try_from(bits).ok()),
                            };
                            if let Some(count) = count {
                                deref_bytes = q.layout.alloc_size(types, *allocated).saturating_mul(count);
                                can_be_null = false;
                            }
                        }
                    }
                    _ => {}
                }
            }
        },
        Operand::Constant(id) => {
            if let ConstantKind::Global(global) = q.context().get(id).kind
                && let Some(GlobalValue { kind: GlobalKind::Variable(variable), linkage, .. }) =
                    q.globals.get(global.0 as usize)
                && is_sized(q, variable.ty)
                && !matches!(linkage, Linkage::ExternWeak)
            {
                // TODO: Don't outright reject hasExternalWeakLinkage but set
                // the CanBeNull flag.
                deref_bytes = q.layout.store_size(types, variable.ty);
                can_be_null = false;
            }
        }
        Operand::Block(_) => {}
    }
    (deref_bytes, can_be_null)
}

/// `Log2(Align)` low bits of a pointer of `width` bits are zero.
fn alignment_zero_bits(
    alignment: u64,
    width: u32,
) -> u128 {
    mask(alignment.max(1).ilog2().min(width))
}

/// The `Call` and `Invoke` case of `computeKnownBitsFromOperator`.
fn known_bits_of_call(
    q: &Query,
    inst: InstId,
    instruction: &Instruction,
    known: &mut KnownBits,
    depth: u32,
) {
    let bit_width = known.width;
    let kb = |v: Operand| compute_known_bits(q, v, depth + 1);

    if let Some(node) = range_metadata(q, instruction) {
        compute_known_bits_from_range_metadata(q, node, known);
    }

    if let Some(range) = call_range(q, instruction) {
        *known = known.union_with(&range.to_known_bits());
    }

    if let Some(rv) = returned_argument_operand(q, instruction) {
        if q.type_of(rv) == Some(instruction.ty) {
            let known2 = compute_known_bits(q, rv, depth + 1);
            *known = known.union_with(&known2);
            if known.has_conflict() {
                known.reset_all();
            }
        }
    }

    let Some(id) = intrinsic_id(q, inst) else { return };
    let args = call_arguments(instruction);
    let arity = match id {
        IntrinsicId::Vscale => 0,
        IntrinsicId::Bitreverse | IntrinsicId::Bswap | IntrinsicId::Ctpop => 1,
        IntrinsicId::Fshl | IntrinsicId::Fshr | IntrinsicId::RiscvVsetvli => 3,
        IntrinsicId::Assume => return,
        _ => 2,
    };
    if args.len() != arity {
        return;
    }
    let bit_width_of = |count: u32| -> u32 { 32 - count.leading_zeros() };

    match id {
        IntrinsicId::Abs => {
            let known2 = kb(args[0]);
            let int_min_is_poison = one(q, args[1]);
            *known = known.union_with(&known2.abs(int_min_is_poison));
        }
        IntrinsicId::Bitreverse => {
            let known2 = kb(args[0]);
            *known = known.union_with(&known2.reverse_bits());
        }
        IntrinsicId::Bswap => {
            let known2 = kb(args[0]);
            *known = known.union_with(&known2.byte_swap());
        }
        IntrinsicId::Ctlz => {
            let known2 = kb(args[0]);
            let mut possible_lz = known2.count_max_leading_zeros();
            if q.int_constant(args[1]) == Some((1, 1)) {
                possible_lz = possible_lz.min(bit_width - 1);
            }
            known.zero = set_range(known.zero, bit_width_of(possible_lz), bit_width);
        }
        IntrinsicId::Cttz => {
            let known2 = kb(args[0]);
            let mut possible_tz = known2.count_max_trailing_zeros();
            if q.int_constant(args[1]) == Some((1, 1)) {
                possible_tz = possible_tz.min(bit_width - 1);
            }
            known.zero = set_range(known.zero, bit_width_of(possible_tz), bit_width);
        }
        IntrinsicId::Ctpop => {
            let known2 = kb(args[0]);
            let bits_possibly_set = known2.count_max_population();
            known.zero = set_range(known.zero, bit_width_of(bits_possibly_set), bit_width);
        }
        IntrinsicId::Fshr | IntrinsicId::Fshl => {
            let Some((sa, _)) = apint(q, args[2]) else { return };
            let known2 = kb(args[0]);
            let known3 = kb(args[1]);
            *known = if id == IntrinsicId::Fshl {
                KnownBits::fshl(&known2, &known3, sa)
            } else {
                KnownBits::fshr(&known2, &known3, sa)
            };
        }
        IntrinsicId::Clmul => *known = KnownBits::clmul(&kb(args[0]), &kb(args[1])),
        IntrinsicId::Pext => *known = KnownBits::pext(&kb(args[0]), &kb(args[1])),
        IntrinsicId::Pdep => *known = KnownBits::pdep(&kb(args[0]), &kb(args[1])),
        IntrinsicId::Smulh if bit_width <= 64 => *known = KnownBits::mulhs(&kb(args[0]), &kb(args[1])),
        IntrinsicId::Umulh if bit_width <= 64 => *known = KnownBits::mulhu(&kb(args[0]), &kb(args[1])),
        IntrinsicId::UaddSat => *known = KnownBits::uadd_sat(&kb(args[0]), &kb(args[1])),
        IntrinsicId::UsubSat => *known = KnownBits::usub_sat(&kb(args[0]), &kb(args[1])),
        IntrinsicId::SaddSat => *known = KnownBits::sadd_sat(&kb(args[0]), &kb(args[1])),
        IntrinsicId::SsubSat => *known = KnownBits::ssub_sat(&kb(args[0]), &kb(args[1])),
        IntrinsicId::Umin => *known = KnownBits::umin(&kb(args[0]), &kb(args[1])),
        IntrinsicId::Umax => *known = KnownBits::umax(&kb(args[0]), &kb(args[1])),
        IntrinsicId::Smin => {
            *known = KnownBits::smin(&kb(args[0]), &kb(args[1]));
            union_with_min_max_intrinsic_clamp(q, inst, known);
        }
        IntrinsicId::Smax => {
            *known = KnownBits::smax(&kb(args[0]), &kb(args[1]));
            union_with_min_max_intrinsic_clamp(q, inst, known);
        }
        IntrinsicId::Ptrmask => {
            *known = kb(args[0]);
            let known2 = kb(args[1]);
            *known = known.and(&known2.anyext_or_trunc(bit_width));
        }
        IntrinsicId::X86Sse42Crc32_64_64 => {
            known.zero = set_range(known.zero, 32, bit_width);
        }
        IntrinsicId::RiscvVsetvli | IntrinsicId::RiscvVsetvlimax => {
            let has_avl = id == IntrinsicId::RiscvVsetvli;
            let range = get_vscale_range(bit_width);
            let (Some((vsew, _)), Some((vlmul, _))) =
                (apint(q, args[usize::from(has_avl)]), apint(q, args[1 + usize::from(has_avl)]))
            else {
                return;
            };
            let sew = 1_u64 << (vsew as u32 + 3);
            let lmul = match vlmul {
                0..=3 => (1_u64 << vlmul) * 8,
                5..=7 => 8 / (1_u64 << (8 - vlmul as u32)),
                _ => return,
            };
            let ratio = (sew * 8) / lmul;
            let max_vlen = (range.get_unsigned_max() as u64).wrapping_mul(64);
            let mut max_vl = max_vlen / ratio;

            if has_avl {
                if let Some((avl, _)) = apint(q, args[0]) {
                    max_vl = max_vl.min(avl as u64);
                }
            }

            let known_zero_first_bit = 31_u32.wrapping_sub((max_vl as u32).leading_zeros()).wrapping_add(1);
            if bit_width > known_zero_first_bit {
                known.zero = set_range(known.zero, known_zero_first_bit, bit_width);
            }
        }
        IntrinsicId::AmdgcnMbcntHi | IntrinsicId::AmdgcnMbcntLo => {
            known.zero = set_range(known.zero, if id == IntrinsicId::AmdgcnMbcntLo { 6 } else { 5 }, bit_width);
            let known2 = kb(args[1]);
            *known = KnownBits::add(known, &known2, false, false, false);
        }
        IntrinsicId::Vscale => {
            *known = get_vscale_range(bit_width).to_known_bits();
        }
        _ => {}
    }
}

/// `computeKnownBitsFromOperator(I, DemandedElts, Known, Q, Depth)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L1461>
fn compute_known_bits_from_operator(
    q: &Query,
    inst: Option<InstId>,
    instruction: &Instruction,
    known: &mut KnownBits,
    depth: u32,
) {
    let bit_width = known.width;
    let operand = |at: usize| instruction.operands[at];
    let kb = |v: Operand| compute_known_bits(q, v, depth + 1);
    let nuw = instruction.flags.contains(Flags::NUW);
    let nsw = instruction.flags.contains(Flags::NSW);
    let exact = instruction.flags.contains(Flags::EXACT);

    match &instruction.opcode {
        Opcode::Load { .. } => {
            if let Some(node) = range_metadata(q, instruction) {
                compute_known_bits_from_range_metadata(q, node, known);
            }
        }
        Opcode::Binary(BinaryOp::And | BinaryOp::Or | BinaryOp::Xor) => {
            let rhs = kb(operand(1));
            let lhs = kb(operand(0));
            *known = get_known_bits_from_and_xor_or(q, instruction, &lhs, &rhs, depth);
        }
        Opcode::Binary(BinaryOp::Mul) => {
            *known = compute_known_bits_mul(q, operand(0), operand(1), nsw, nuw, depth);
        }
        Opcode::Binary(BinaryOp::UDiv) => {
            let lhs = kb(operand(0));
            *known = KnownBits::udiv(&lhs, &kb(operand(1)), exact);
        }
        Opcode::Binary(BinaryOp::SDiv) => {
            let lhs = kb(operand(0));
            *known = KnownBits::sdiv(&lhs, &kb(operand(1)), exact);
        }
        Opcode::Select => {
            let compute_for_arm = |arm: Operand, invert: bool| {
                let mut res = kb(arm);
                adjust_known_bits_for_select_arm(q, &mut res, operand(0), arm, invert, depth);
                res
            };
            *known = compute_for_arm(operand(1), false).intersect_with(&compute_for_arm(operand(2), true));
        }
        Opcode::Cast(CastOp::PtrToInt | CastOp::IntToPtr | CastOp::ZExt | CastOp::Trunc) => {
            *known = kb(operand(0));
            if matches!(instruction.opcode, Opcode::Cast(CastOp::ZExt))
                && instruction.flags.contains(Flags::NNEG)
                && !known.is_negative()
            {
                known.make_non_negative();
            }
            *known = known.zext_or_trunc(bit_width);
        }
        Opcode::Cast(CastOp::BitCast) => {
            let scalar = |ty: Option<TypeId>| {
                ty.is_some_and(|ty| matches!(q.context().types.get(ty), Type::Int(_) | Type::Pointer(_)))
            };
            if scalar(q.type_of(operand(0))) && !matches!(q.context().types.get(instruction.ty), Type::Vector { .. }) {
                *known = kb(operand(0));
            }
        }
        Opcode::Cast(CastOp::SExt) => {
            *known = kb(operand(0));
            *known = known.sext(bit_width);
        }
        Opcode::Binary(BinaryOp::Shl) => {
            let kf = |value: &KnownBits, amount: &KnownBits, non_zero: bool| {
                KnownBits::shl(value, amount, nuw, nsw, non_zero)
            };
            *known = compute_known_bits_from_shift_operator(q, instruction, depth, &kf);
            if let Some((c, width)) = apint(q, operand(0)) {
                known.zero |= mask(countr_zero(c, width));
            }

            let yc = Cell::new(None);
            let x = operand(0);
            let none = Flags::default();
            if bit_width.is_power_of_two()
                && match_binary(q, operand(1), BinaryOp::Sub, none, false, &bind_apint(q, &yc), &|n| {
                    match_binary(
                        q,
                        n,
                        BinaryOp::Xor,
                        none,
                        false,
                        &|c| match_intrinsic(q, c, IntrinsicId::Ctlz, false, &exactly(x), &|o| one(q, o)),
                        &|c| specific_int(q, c, u128::from(bit_width - 1)),
                    )
                })
                && yc.get().is_some_and(|(y, _)| y < u128::from(bit_width - 1))
            {
                let y = yc.get().expect("matched").0 as u32;
                known.one |= 1_u128 << y;
                known.zero = set_range(known.zero, y + 1, bit_width);
            }
        }
        Opcode::Binary(BinaryOp::LShr) => {
            let kf =
                |value: &KnownBits, amount: &KnownBits, non_zero: bool| KnownBits::lshr(value, amount, non_zero, exact);
            *known = compute_known_bits_from_shift_operator(q, instruction, depth, &kf);
            if let Some((c, width)) = apint(q, operand(0)) {
                known.zero = set_high(known.zero, bit_width, countl_zero(c, width));
            }
        }
        Opcode::Binary(BinaryOp::AShr) => {
            let kf =
                |value: &KnownBits, amount: &KnownBits, non_zero: bool| KnownBits::ashr(value, amount, non_zero, exact);
            *known = compute_known_bits_from_shift_operator(q, instruction, depth, &kf);
        }
        Opcode::Binary(BinaryOp::Sub) => {
            *known = compute_known_bits_add_sub(q, false, operand(0), operand(1), nsw, nuw, depth);
        }
        Opcode::Binary(BinaryOp::Add) => {
            *known = compute_known_bits_add_sub(q, true, operand(0), operand(1), nsw, nuw, depth);
        }
        Opcode::Binary(BinaryOp::SRem) => {
            let lhs = kb(operand(0));
            *known = KnownBits::srem(&lhs, &kb(operand(1)));
        }
        Opcode::Binary(BinaryOp::URem) => {
            let lhs = kb(operand(0));
            *known = KnownBits::urem(&lhs, &kb(operand(1)));
        }
        Opcode::Alloca { .. } => {
            known.zero |= alignment_zero_bits(pointer_alignment(q, result_of(instruction)), bit_width);
        }
        Opcode::GetElementPtr { source } => {
            known_bits_of_get_element_ptr(q, instruction, *source, known, depth);
        }
        Opcode::Phi => {
            known_bits_of_phi(q, inst.expect("an instruction"), instruction, known, depth);
        }
        Opcode::Call(_) | Opcode::Invoke(_) => {
            known_bits_of_call(q, inst.expect("an instruction"), instruction, known, depth);
        }
        Opcode::ExtractValue(indices) => {
            if let Some((ii, call)) = q.instruction_of(operand(0)).filter(|(inst, _)| is_intrinsic(q, *inst)) {
                if indices.len() == 1 && indices[0] == 0 && call_arguments(call).len() == 2 {
                    let arguments = call_arguments(call);
                    match intrinsic_id(q, ii) {
                        Some(IntrinsicId::UaddWithOverflow | IntrinsicId::SaddWithOverflow) => {
                            *known =
                                compute_known_bits_add_sub(q, true, arguments[0], arguments[1], false, false, depth);
                        }
                        Some(IntrinsicId::UsubWithOverflow | IntrinsicId::SsubWithOverflow) => {
                            *known =
                                compute_known_bits_add_sub(q, false, arguments[0], arguments[1], false, false, depth);
                        }
                        Some(IntrinsicId::UmulWithOverflow | IntrinsicId::SmulWithOverflow) => {
                            *known = compute_known_bits_mul(q, arguments[0], arguments[1], false, false, depth);
                        }
                        _ => {}
                    }
                }
            }
        }
        Opcode::Freeze => {
            if is_guaranteed_not_to_be_poison(q, operand(0), depth + 1) {
                *known = kb(operand(0));
            }
        }
        _ => {}
    }
}

/// A constant expression as the instruction it is (`isa<Operator>`).
fn constant_expression(
    q: &Query,
    v: Operand,
) -> Option<Instruction> {
    let Operand::Constant(id) = v else { return None };
    let constant = q.context().get(id);
    let ConstantKind::Expr(expr) = &constant.kind else { return None };
    let (opcode, operands, flags) = match expr {
        ConstantExpr::Cast { op, value } => (Opcode::Cast(*op), vec![Operand::Constant(*value)], Flags::default()),
        ConstantExpr::Binary { op, lhs, rhs } => {
            (Opcode::Binary(*op), vec![Operand::Constant(*lhs), Operand::Constant(*rhs)], Flags::default())
        }
        ConstantExpr::GetElementPtr { source, inbounds, operands, .. } => (
            Opcode::GetElementPtr { source: *source },
            operands.iter().map(|operand| Operand::Constant(*operand)).collect(),
            if *inbounds { Flags::INBOUNDS } else { Flags::default() },
        ),
    };
    Some(Instruction { opcode, ty: constant.ty, operands, flags, result: None, metadata: Vec::new(), link: (0, 0) })
}

/// `computeKnownBits(V, DemandedElts, Known, Q, Depth)`: `known` has the width
/// of `v`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L2550>
pub fn compute_known_bits_into(
    q: &Query,
    v: Operand,
    known: &mut KnownBits,
    depth: u32,
) {
    // The operand's kind is read once: a value is no constant, as an
    // `isa<Constant>` costs C++ nothing.
    let ty = match v {
        Operand::Constant(id) => {
            if let Some((bits, width)) = q.int_constant(v) {
                *known = KnownBits::constant(width, bits);
                return;
            }
            let constant = q.context().get(id);
            match constant.kind {
                ConstantKind::Null | ConstantKind::Zero => {
                    known.set_all_zero();
                    return;
                }
                // We can't imply anything about undefs.
                ConstantKind::Poison | ConstantKind::Undef => {
                    known.reset_all();
                    return;
                }
                _ => {}
            }
            known.reset_all();
            Some(constant.ty)
        }
        Operand::Value(id) => {
            known.reset_all();
            let value = q.function.value(id);
            if let ValueDef::Argument(index) = value.def {
                if let Some(range) = argument_range(q, index) {
                    *known = range.to_known_bits();
                }
            }
            Some(value.ty)
        }
        Operand::Block(_) => {
            known.reset_all();
            None
        }
    };

    if depth == MAX_ANALYSIS_RECURSION_DEPTH {
        return;
    }

    if let Some((inst, instruction)) = q.instruction_of(v) {
        compute_known_bits_from_operator(q, Some(inst), instruction, known, depth);
    } else if let Some(operator) = constant_expression(q, v) {
        compute_known_bits_from_operator(q, None, &operator, known, depth);
    }

    if ty.is_some_and(|ty| matches!(q.context().types.get(ty), Type::Pointer(_))) {
        known.zero |= alignment_zero_bits(pointer_alignment(q, v), known.width);
    }

    compute_known_bits_from_context(q, v, known, depth);
}

/// `computeKnownBits(V, Q, Depth)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L2528>
pub fn compute_known_bits(
    q: &Query,
    v: Operand,
    depth: u32,
) -> KnownBits {
    let mut known = KnownBits::unknown(width_of(q, v));
    compute_known_bits_into(q, v, &mut known, depth);
    known
}

/// The `kb` column of `examples/vtdump.rs`.
pub fn column(
    q: &Query,
    v: Operand,
) -> String {
    format!(" kb={}", compute_known_bits(q, v, 0).print())
}

/// The `nn` column of `examples/vtdump.rs`.
pub fn column_non_negative(
    q: &Query,
    v: Operand,
) -> String {
    format!(" nn={}", u8::from(is_known_non_negative(q, v, 0)))
}

/// The `ng` column of `examples/vtdump.rs`.
pub fn column_negative(
    q: &Query,
    v: Operand,
) -> String {
    format!(" ng={}", u8::from(is_known_negative(q, v, 0)))
}
