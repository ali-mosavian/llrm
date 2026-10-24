//! `icmp (A + B), Op1` and its kin: the folds of `foldICmpBinOp` that read a
//! sum, from
//! [`InstCombineCompares.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp)
//! at d1106deb71cc, the block of
//! [`foldICmpBinOp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L5210)
//! from the wrap analysis
//! ([L5397](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L5397))
//! to the strictness flips
//! ([L5517](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L5517)).
//!
//! MIRROR: the order of the folds and each condition.
//!
//! OWED, at its place in [`fold_icmp_bin_op`]: the rest of `foldICmpBinOp`
//! (the folds before the wrap analysis and the ones after the strictness
//! flips).
//!
//! DEVIATE 1 (temporary): `ShareCommonDivisor` reads `isMultipleOf` of both
//! sides for an offset other than 1 and -1; here only the fast path
//! (`Z == 1` or `Z == -1`) is made. Exit: `isMultipleOf` translated in
//! `valuetracking`.
//!
//! # How it works
//! A sum that cannot wrap for the compare's signedness (an equality needs no
//! flag; an `or disjoint` is an `add nuw nsw`) cancels against the other side:
//! `x + y == x` is `y == 0`, `x + y == x + z` is `y == z`. A relational compare
//! of a sum of one against a value turns the strictness: `x + 1 s<= y` is
//! `x s< y`.
//!
//! # Used by
//! `combine_compares::visit_icmp`.
//!
//! # Example
//! The loop test of `sift`, `2 * i + 1 <= n`:
//!
//! ```text
//! %o = or disjoint i32 %x, 1
//! %c = icmp sle i32 %o, %n         ; %c = icmp slt i32 %x, %n
//! ```

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;

/// `ICmpInst::isSigned`.
fn is_signed(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
    )
}

/// `ICmpInst::isUnsigned`.
fn is_unsigned(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Ugt | IntPredicate::Uge | IntPredicate::Ult | IntPredicate::Ule
    )
}

/// `ICmpInst::isLT`: `<` or `<=` in either signedness.
fn is_lt(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Slt | IntPredicate::Ult)
}

/// `ICmpInst::isGE`.
fn is_ge(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Sge | IntPredicate::Uge)
}

/// `ICmpInst::isGT`.
fn is_gt(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Sgt | IntPredicate::Ugt)
}

/// `ICmpInst::isLE`.
fn is_le(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Sle | IntPredicate::Ule)
}

/// `CmpInst::getFlippedStrictnessPredicate`: `<` for `<=`, `>=` for `>`.
fn flipped_strictness(pred: IntPredicate) -> IntPredicate {
    match pred {
        IntPredicate::Slt => IntPredicate::Sle,
        IntPredicate::Sle => IntPredicate::Slt,
        IntPredicate::Sgt => IntPredicate::Sge,
        IntPredicate::Sge => IntPredicate::Sgt,
        IntPredicate::Ult => IntPredicate::Ule,
        IntPredicate::Ule => IntPredicate::Ult,
        IntPredicate::Ugt => IntPredicate::Uge,
        IntPredicate::Uge => IntPredicate::Ugt,
        same => same,
    }
}

/// One side of the compare, seen as `foldICmpBinOp` sees it: the two operands
/// of an `m_AddLike` (`None` when it is not one) and whether the sum cannot
/// wrap for the compare (`hasNoWrapProblem`).
struct Side {
    operands: Option<(Operand, Operand)>,
    no_wrap_problem: bool,
}

/// The `A + B` and `NoOpWrapProblem` of `operand` for `pred`.
fn side(
    c: &Combiner,
    operand: Operand,
    pred: IntPredicate,
) -> Side {
    let q = query(c, None);
    let none = Side { operands: None, no_wrap_problem: false };
    let Some((_, instruction)) = q.instruction_of(operand) else { return none };
    let Opcode::Binary(op) = instruction.opcode else { return none };
    let operands = pattern::add_like(&q, operand, Flags::default());
    let no_wrap_problem = match op {
        // `OverflowingBinaryOperator`.
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl => {
            matches!(pred, IntPredicate::Eq | IntPredicate::Ne)
                || (is_unsigned(pred) && instruction.flags.contains(Flags::NUW))
                || (is_signed(pred) && instruction.flags.contains(Flags::NSW))
        }
        // The invariant here is that `m_AddLike` is handled, which can only be
        // an `or disjoint`, which is an `add nuw nsw`.
        BinaryOp::Or => true,
        _ => false,
    };
    Side { operands, no_wrap_problem }
}

/// `ShareCommonDivisor` for the offset `z`: both sides are multiples of it,
/// which holds for 1 (and -1 when `negative`).
fn share_common_divisor(
    c: &Combiner,
    z: Operand,
    negative: bool,
) -> bool {
    let Some((bits, width)) = query(c, None).int_constant(z) else { return false };
    let all_ones = llrm_mir::context::mask(width);
    // Fast path for `Z == 1` / `-1`; the general offset is DEVIATE 1.
    if negative { bits == all_ones } else { bits == 1 }
}

/// The sum folds of `foldICmpBinOp` on `inst`.
pub fn fold_icmp_bin_op(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let left = side(c, op0, pred);
    let right = side(c, op1, pred);
    let (a, b) = left.operands.unzip();
    let (cc, d) = right.operands.unzip();
    // icmp (A+B), A -> icmp B, 0 for equalities or if there is no overflow.
    // icmp (A+B), B -> icmp A, 0 for equalities or if there is no overflow.
    if (a == Some(op1) || b == Some(op1)) && left.no_wrap_problem {
        let other = if a == Some(op1) { b } else { a }.expect("a sum");
        let zero = c.int(c.type_of(op1), 0);
        return Some(new_icmp(c, inst, pred, other, zero));
    }
    // icmp C, (C+D) -> icmp 0, D for equalities or if there is no overflow.
    // icmp D, (C+D) -> icmp 0, C for equalities or if there is no overflow.
    if (cc == Some(op0) || d == Some(op0)) && right.no_wrap_problem {
        let other = if cc == Some(op0) { d } else { cc }.expect("a sum");
        let zero = c.int(c.type_of(op0), 0);
        return Some(new_icmp(c, inst, pred, zero, other));
    }
    // icmp (A+B), (A+D) -> icmp B, D for equalities or if there is no overflow.
    if let (Some(a), Some(b), Some(cc), Some(d)) = (a, b, cc, d)
        && (a == cc || a == d || b == cc || b == d)
        && left.no_wrap_problem
        && right.no_wrap_problem
    {
        let (y, z) = if a == cc {
            (b, d)
        } else if a == d {
            (b, cc)
        } else if b == cc {
            (a, d)
        } else {
            (a, cc)
        };
        return Some(new_icmp(c, inst, pred, y, z));
    }
    if !matches!(pred, IntPredicate::Eq | IntPredicate::Ne) {
        // icmp slt (A + -1), Op1 -> icmp sle A, Op1
        // icmp sge (A + -1), Op1 -> icmp sgt A, Op1
        // icmp sle (A + 1), Op1 -> icmp slt A, Op1
        // icmp sgt (A + 1), Op1 -> icmp sge A, Op1
        // icmp ule (A + 1), Op0 -> icmp ult A, Op1
        // icmp ugt (A + 1), Op0 -> icmp uge A, Op1
        let negative = is_lt(pred) || is_ge(pred);
        let add_or_signed = !negative || is_signed(pred);
        if let Some((a, b)) = left.operands
            && left.no_wrap_problem
            && add_or_signed
            && share_common_divisor(c, b, negative)
        {
            return Some(new_icmp(c, inst, flipped_strictness(pred), a, op1));
        }
        // icmp sgt Op0, (C + -1) -> icmp sge Op0, C
        // icmp sle Op0, (C + -1) -> icmp slt Op0, C
        // icmp sge Op0, (C + 1) -> icmp sgt Op0, C
        // icmp slt Op0, (C + 1) -> icmp sle Op0, C
        // icmp uge Op0, (C + 1) -> icmp ugt Op0, C
        // icmp ult Op0, (C + 1) -> icmp ule Op0, C
        if let Some((cc, d)) = right.operands
            && right.no_wrap_problem
            && share_common_divisor(c, d, is_gt(pred) || is_le(pred))
        {
            return Some(new_icmp(c, inst, flipped_strictness(pred), op0, cc));
        }
    }
    None
}

/// A new compare in place of `inst`: `new ICmpInst(pred, a, b)`.
fn new_icmp(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    a: Operand,
    b: Operand,
) -> Visit {
    let i1 = c.context.types.int(1);
    c.replace_with_new(inst, Opcode::ICmp(pred), i1, vec![a, b], Flags::default())
}

#[cfg(test)]
#[path = "combine_icmp_binop_tests.rs"]
mod tests;
