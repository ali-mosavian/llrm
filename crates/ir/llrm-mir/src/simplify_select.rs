//! InstSimplify of selects on a compare: `simplifySelectWithICmpCond`,
//! `simplifySelectWithBitTest`, `simplifySelectBitTest`,
//! `simplifySelectWithEquivalence`, `simplifyCmpSelOfMaxMin`,
//! `isSelectWithIdenticalPHI`, and `decomposeBitTest`.
//!
//! A translation of LLVM's
//! [`InstructionSimplify.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp)
//! (`simplifyCmpSelOfMaxMin`
//! [:4732](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L4732),
//! `simplifySelectBitTest` :4692, `simplifySelectWithEquivalence` :4817,
//! `simplifySelectWithICmpCond` :4842, `isSelectWithIdenticalPHI` :5053) and
//! [`CmpInstAnalysis.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/CmpInstAnalysis.cpp#L77)
//! (`decomposeBitTestICmp`, `decomposeBitTest`). A child of `simplify.rs`,
//! whose `Query` it shares. Each function keeps its C++ order of checks.
//!
//! DEVIATE, permanent (MIR lacks the construct): vector selects and the
//! shuffle in `simplifyCmpSelOfMaxMin` (#2201).
//!
//! OWED: `canReplacePointersIfEqual` (an equality of pointers replaces one by
//! the other only through `simplifySelectWithEquivalence` for integers here,
//! #2247); `simplifySelectWithFCmp` (#2246); `simplifyICmpWithDominatingAssume`
//! (#2249, in the compare, not the select).
//!
//! Used by `simplify_select`.

use super::*;
use crate::valuetracking::bittest::{BitTestOptions, decompose_bit_test};
use crate::valuetracking::pattern::IntrinsicId;
use crate::valuetracking::range::{SelectOperand, SelectPatternFlavor, get_inverse_min_max_flavor, get_min_max_pred};

/// `CmpInst::getStrictPredicate`.
fn strict(pred: IntPredicate) -> IntPredicate {
    match pred {
        IntPredicate::Sge => IntPredicate::Sgt,
        IntPredicate::Sle => IntPredicate::Slt,
        IntPredicate::Uge => IntPredicate::Ugt,
        IntPredicate::Ule => IntPredicate::Ult,
        other => other,
    }
}

/// `simplifySelectBitTest`.
fn simplify_select_bit_test(
    q: &Query,
    true_val: Operand,
    false_val: Operand,
    x: Operand,
    y: u128,
    true_when_unset: bool,
) -> Option<Operand> {
    let width = q.width(x)?;
    let not_y = !y & mask(width);
    let and_with = |value: Operand| match q.binary_of(value, BinaryOp::And) {
        Some((a, b, _)) if a == x => q.number(b).map(|(bits, _)| bits),
        _ => None,
    };
    let or_with = |value: Operand| match q.binary_of(value, BinaryOp::Or) {
        Some((a, b, flags)) if a == x => q.number(b).map(|(bits, _)| (bits, flags.contains(Flags::DISJOINT))),
        _ => None,
    };
    let pick = |when_unset: Operand, when_set: Operand| if true_when_unset { when_unset } else { when_set };
    // (X & Y) == 0 ? X & ~Y : X  --> X
    // (X & Y) != 0 ? X & ~Y : X  --> X & ~Y
    if false_val == x && and_with(true_val) == Some(not_y) {
        return Some(pick(false_val, true_val));
    }
    // (X & Y) == 0 ? X : X & ~Y  --> X & ~Y
    // (X & Y) != 0 ? X : X & ~Y  --> X
    if true_val == x && and_with(false_val) == Some(not_y) {
        return Some(pick(false_val, true_val));
    }
    if y.is_power_of_two() {
        // (X & Y) == 0 ? X | Y : X  --> X | Y
        // (X & Y) != 0 ? X | Y : X  --> X
        if false_val == x
            && let Some((c, disjoint)) = or_with(true_val)
            && c == y
        {
            // We can't return the or if it has the disjoint flag.
            if true_when_unset && disjoint {
                return None;
            }
            return Some(if true_when_unset { true_val } else { false_val });
        }
        // (X & Y) == 0 ? X : X | Y  --> X
        // (X & Y) != 0 ? X : X | Y  --> X | Y
        if true_val == x
            && let Some((c, disjoint)) = or_with(false_val)
            && c == y
        {
            if !true_when_unset && disjoint {
                return None;
            }
            return Some(if true_when_unset { true_val } else { false_val });
        }
    }
    None
}

/// `simplifySelectWithBitTest`.
pub(super) fn simplify_select_with_bit_test(
    q: &Query,
    cond: Operand,
    true_val: Operand,
    false_val: Operand,
) -> Option<Operand> {
    let decomposed = decompose_bit_test(&q.tracking(), cond, BitTestOptions::DEFAULT)?;
    simplify_select_bit_test(q, true_val, false_val, decomposed.x, decomposed.mask, decomposed.pred == IntPredicate::Eq)
}

/// `simplifyCmpSelOfMaxMin`, scalar.
fn simplify_cmp_sel_of_max_min(
    q: &Query,
    mut cmp_lhs: Operand,
    mut cmp_rhs: Operand,
    mut pred: IntPredicate,
    mut t_val: Operand,
    mut f_val: Operand,
) -> Option<Operand> {
    // Canonicalize common cmp+sel operand as CmpLHS.
    if cmp_rhs == t_val || cmp_rhs == f_val {
        std::mem::swap(&mut cmp_lhs, &mut cmp_rhs);
        pred = swapped(pred);
    }
    // Canonicalize common cmp+sel operand as TVal.
    if cmp_lhs == f_val {
        std::mem::swap(&mut t_val, &mut f_val);
        pred = inverse(pred);
    }
    let (x, y) = (cmp_lhs, cmp_rhs);
    // (X pred Y) ? X : max/min(X, Y)
    let (id, a, b) = calls::max_or_min(q, f_val)?;
    if t_val != x || !((a == x && b == y) || (a == y && b == x)) {
        return None;
    }
    let mm_pred = match id {
        IntrinsicId::Smax => IntPredicate::Sgt,
        IntrinsicId::Smin => IntPredicate::Slt,
        IntrinsicId::Umax => IntPredicate::Ugt,
        _ => IntPredicate::Ult,
    };
    // (X >  Y) ? X : max(X, Y) --> max(X, Y), and the three kin.
    if mm_pred == strict(pred) {
        return Some(f_val);
    }
    // (X == Y) ? X : max/min(X, Y) --> max/min(X, Y)
    if pred == IntPredicate::Eq {
        return Some(f_val);
    }
    // (X != Y) ? X : max/min(X, Y) --> X
    if pred == IntPredicate::Ne {
        return Some(x);
    }
    // (X <  Y) ? X : max(X, Y) --> X, and the three kin.
    if mm_pred == strict(inverse(pred)) {
        return Some(x);
    }
    None
}

/// `simplifySelectWithEquivalence`.
fn simplify_select_with_equivalence(
    q: &mut Query,
    replacements: &[(Operand, Operand)],
    true_val: Operand,
    false_val: Operand,
    max_recurse: u32,
) -> Option<Operand> {
    let could_use_undef = std::mem::replace(&mut q.can_use_undef, false);
    let simplified_false = simplify_with_ops_replaced(q, false_val, replacements, false, max_recurse);
    q.can_use_undef = could_use_undef;
    let simplified_false = simplified_false.unwrap_or(false_val);
    let simplified_true = simplify_with_ops_replaced(q, true_val, replacements, true, max_recurse).unwrap_or(true_val);
    (simplified_false == simplified_true).then_some(false_val)
}

/// A call of `fshl` or `fshr`: the intrinsic and its arguments.
fn funnel_shift(
    q: &Query,
    value: Operand,
) -> Option<(IntrinsicId, Operand, Operand, Operand)> {
    let tracking = q.tracking();
    let (inst, _) = tracking.instruction_of(value)?;
    let id = pattern::call_intrinsic(&tracking, inst)?;
    if !matches!(id, IntrinsicId::Fshl | IntrinsicId::Fshr) {
        return None;
    }
    let [a, b, c, _] = q.function.instruction(inst).operands[..] else { return None };
    Some((id, a, b, c))
}

/// `m_Intrinsic<Intrinsic::abs>(m_Specific(x))`.
fn is_abs_of(
    q: &Query,
    value: Operand,
    x: Operand,
) -> bool {
    let tracking = q.tracking();
    pattern::intrinsic_call(&tracking, value, "llvm.abs").is_some_and(|(_, args)| args.first() == Some(&x))
}

/// `simplifySelectWithICmpCond`.
pub(super) fn simplify_select_with_icmp_cond(
    q: &mut Query,
    cond: Operand,
    mut true_val: Operand,
    mut false_val: Operand,
    max_recurse: u32,
) -> Option<Operand> {
    let cmp = q.definition(cond)?;
    let instruction = q.function.instruction(cmp);
    let (Opcode::ICmp(pred), [cmp_lhs, cmp_rhs]) = (&instruction.opcode, &instruction.operands[..]) else {
        return None;
    };
    let (mut pred, cmp_lhs, cmp_rhs) = (*pred, *cmp_lhs, *cmp_rhs);
    if let Some(v) = simplify_cmp_sel_of_max_min(q, cmp_lhs, cmp_rhs, pred, true_val, false_val) {
        return Some(v);
    }
    // Canonicalize ne to eq predicate.
    if pred == IntPredicate::Ne {
        pred = IntPredicate::Eq;
        std::mem::swap(&mut true_val, &mut false_val);
    }
    // Check for integer min/max with a limit constant:
    // X > MIN_INT ? X : MIN_INT --> X
    // X < MAX_INT ? X : MAX_INT --> X
    if q.width(true_val).is_some() {
        let result = range::match_decomposed_select_pattern(
            &q.tracking(),
            cmp,
            true_val,
            false_val,
            Default::default(),
            true,
            0,
        );
        let flavor = result.flavor;
        if matches!(
            flavor,
            SelectPatternFlavor::SMin
                | SelectPatternFlavor::SMax
                | SelectPatternFlavor::UMin
                | SelectPatternFlavor::UMax
        ) && get_min_max_pred(flavor, false) == range::Predicate::Int(pred)
            && let (Some(SelectOperand::Value(x)), Some(rhs)) = (result.lhs, result.rhs)
        {
            let width = q.width(x)?;
            let limit_id = match get_inverse_min_max_flavor(flavor) {
                SelectPatternFlavor::SMax => IntrinsicId::Smax,
                SelectPatternFlavor::SMin => IntrinsicId::Smin,
                SelectPatternFlavor::UMax => IntrinsicId::Umax,
                _ => IntrinsicId::Umin,
            };
            let limit = calls::saturation_point(limit_id, width);
            let y_is_limit = match rhs {
                SelectOperand::Int { bits, .. } => bits == limit,
                SelectOperand::Value(y) => q.number(y).is_some_and(|(bits, _)| bits == limit),
                _ => false,
            };
            if y_is_limit {
                return Some(x);
            }
        }
    }
    if pred == IntPredicate::Eq && q.is_zero(cmp_rhs) {
        if let Some((x, y, _)) = q.binary_of(cmp_lhs, BinaryOp::And)
            && let Some((y, _)) = q.number(y)
            && let Some(v) = simplify_select_bit_test(q, true_val, false_val, x, y, true)
        {
            return Some(v);
        }
        // Test for a bogus zero-shift-guard-op around funnel-shift or rotate.
        // (ShAmt == 0) ? fshl(X, *, ShAmt) : X --> X
        // (ShAmt == 0) ? fshr(*, X, ShAmt) : X --> X
        if let Some((id, a, b, sh_amt)) = funnel_shift(q, true_val) {
            let x = if id == IntrinsicId::Fshl { a } else { b };
            if false_val == x && cmp_lhs == sh_amt {
                return Some(x);
            }
        }
        // Test for a zero-shift-guard-op around rotates.
        if let Some((_, x, other, sh_amt)) = funnel_shift(q, false_val)
            && x == other
            && true_val == x
        {
            // (ShAmt == 0) ? X : fshl(X, X, ShAmt) --> fshl(X, X, ShAmt)
            if cmp_lhs == sh_amt {
                return Some(false_val);
            }
            let bw = q.width(x)?;
            // ((ShAmt & (BW-1)) == 0) ? X : fshl(X, X, ShAmt) --> fshl(X, X,
            // ShAmt)
            if bw.is_power_of_two()
                && let Some((a, b, _)) = q.binary_of(cmp_lhs, BinaryOp::And)
                && ((a == sh_amt && q.number(b).is_some_and(|(bits, _)| bits == u128::from(bw) - 1))
                    || (b == sh_amt && q.number(a).is_some_and(|(bits, _)| bits == u128::from(bw) - 1)))
            {
                return Some(false_val);
            }
            // (ShAmt % BW == 0) ? X : fshl(X, X, ShAmt) --> fshl(X, X, ShAmt)
            if let Some((a, b, _)) = q.binary_of(cmp_lhs, BinaryOp::URem)
                && a == sh_amt
                && q.number(b).is_some_and(|(bits, _)| bits == u128::from(bw))
            {
                return Some(false_val);
            }
        }
        // X == 0 ? abs(X) : -abs(X) --> -abs(X)
        // X == 0 ? -abs(X) : abs(X) --> abs(X)
        let neg_abs_of =
            |q: &Query, value: Operand| q.neg_of(value).is_some_and(|(inner, _)| is_abs_of(q, inner, cmp_lhs));
        if is_abs_of(q, true_val, cmp_lhs) && neg_abs_of(q, false_val) {
            return Some(false_val);
        }
        if neg_abs_of(q, true_val) && is_abs_of(q, false_val, cmp_lhs) {
            return Some(false_val);
        }
    }
    // If we have a scalar equality comparison, then we know the value in one of
    // the arms of the select. See if substituting this value into the arm and
    // simplifying the result yields the same value as the other arm.
    if pred == IntPredicate::Eq {
        if q.width(cmp_lhs).is_some() {
            if let Some(v) =
                simplify_select_with_equivalence(q, &[(cmp_lhs, cmp_rhs)], true_val, false_val, max_recurse)
            {
                return Some(v);
            }
            if let Some(v) =
                simplify_select_with_equivalence(q, &[(cmp_rhs, cmp_lhs)], true_val, false_val, max_recurse)
            {
                return Some(v);
            }
        }
        // select((X | Y) == 0 ? X : 0) --> 0 (commuted 2 ways)
        if let Some((x, y, _)) = q.binary_of(cmp_lhs, BinaryOp::Or)
            && q.is_zero(cmp_rhs)
            && let Some(v) =
                simplify_select_with_equivalence(q, &[(x, cmp_rhs), (y, cmp_rhs)], true_val, false_val, max_recurse)
        {
            return Some(v);
        }
        // select((X & Y) == -1 ? X : -1) --> -1 (commuted 2 ways)
        if let Some((x, y, _)) = q.binary_of(cmp_lhs, BinaryOp::And)
            && q.is_all_ones(cmp_rhs)
            && let Some(v) =
                simplify_select_with_equivalence(q, &[(x, cmp_rhs), (y, cmp_rhs)], true_val, false_val, max_recurse)
        {
            return Some(v);
        }
    }
    None
}

/// `isSelectWithIdenticalPHI`: `pn` and `identical` are equal at every
/// iteration, as the one value they differ in is a select between them.
pub(super) fn is_select_with_identical_phi(
    q: &Query,
    pn: Operand,
    identical: Operand,
) -> bool {
    let (Some(pn_inst), Some(other_inst)) = (q.definition(pn), q.definition(identical)) else { return false };
    let (a, b) = (q.function.instruction(pn_inst), q.function.instruction(other_inst));
    if q.function.parent(pn_inst) != q.function.parent(other_inst) || a.operands.len() != 4 {
        return false;
    }
    let incoming = |instruction: &crate::module::Instruction, block: Operand| {
        instruction.operands.chunks(2).find(|pair| pair[1] == block).map(|pair| pair[0])
    };
    // Check that only the backedge incoming value is different.
    let mut different = Vec::new();
    for pair in a.operands.chunks(2) {
        if Some(pair[0]) != incoming(b, pair[1]) {
            different.push(pair[1]);
        }
    }
    let [diff_block] = different[..] else { return false };
    // The backedge incoming values are two selects with the same condition.
    let (Some(value), Some(other_value)) = (incoming(a, diff_block), incoming(b, diff_block)) else { return false };
    let select = |value: Operand| {
        let inst = q.definition(value)?;
        let instruction = q.function.instruction(inst);
        (instruction.opcode == Opcode::Select)
            .then(|| (instruction.operands[0], instruction.operands[1], instruction.operands[2]))
    };
    let (Some((cond, t, f)), Some((other_cond, other_t, other_f))) = (select(value), select(other_value)) else {
        return false;
    };
    if cond != other_cond {
        return false;
    }
    // Either their true values are the same, or their false values are.
    let (inner, identical_other) = if t == other_t {
        (f, other_f)
    } else if f == other_f {
        (t, other_t)
    } else {
        return false;
    };
    if identical_other != identical {
        return false;
    }
    let Some((_, inner_t, inner_f)) = select(inner) else { return false };
    (inner_t == identical && inner_f == pn) || (inner_t == pn && inner_f == identical)
}
