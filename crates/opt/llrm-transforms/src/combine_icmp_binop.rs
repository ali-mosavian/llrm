//! InstCombine's `foldICmpBinOp`: the compare of a binary operation, from
//! [`InstCombineCompares.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L5301)
//! at d1106deb71cc.
//!
//! MIRROR: the order of the steps and each condition (the `nsw`/`nuw` rule of
//! `hasNoWrapProblem` included), from the function's start to the end of its
//! `sub` section (`icmp (0-X) < C`).
//!
//! The steps after that stay in their place as `// OWED:` comments, in
//! LLVM's order: `foldICmpXorXX`, `foldICmpOrXX`, the shared multiplier
//! (`X * Z pred Y * Z`), the `srem` against its divisor, the two operations of
//! one opcode with a shared right operand (`a+x`, `a^x`, `a*C`, the exact
//! divisions and shifts), `foldICmpAndXX`, `foldMultiplicationOverflowCheck`,
//! `foldICmpWithTruncSignExtendedVal` and
//! `foldShiftIntoShiftInAnotherHandOfAndInICmp`; and the first step,
//! `foldICmpXNegX`.
//!
//! DEVIATE 1 (permanent): no vectors, no `ConstantExpr`: `m_APInt`,
//! `m_ImmConstant` and `m_SignMask` read scalar integer constants.
//!
//! # Used by
//! [`crate::combine_compares::visit_icmp`], between the `x >u INT_MAX` fold
//! and `foldICmpInstWithConstant`.
//!
//! # How it works
//! An operand that is an `add` (or an `or disjoint`) or a `sub` is split into
//! its parts. A compare of `A+B` against `A`, of two sums that share a term,
//! of two differences that share one, or against a constant that differs from
//! another by a step is the compare of what is left, where the operation
//! cannot wrap in the compare's signedness. A relational compare against
//! `C + -1` or `C + 1` (the step) swaps strict for non-strict, where
//! the other side is a multiple of the step.
//!
//! # Example
//! `a > b + -1` (`nsw`) is `a >= b`; with `b < a` known, the branch on it
//! folds in `SimplifyCFG`:
//!
//! ```text
//! %d = add nsw i16 %b, -1
//! %g = icmp sgt i16 %a, %d   ; %g = icmp sge i16 %a, %b
//! ```

use llrm_mir::context::{mask, signed};
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::{known, nonzero, pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;

/// A binary operation operand (`dyn_cast<BinaryOperator>`).
struct Bin {
    inst: InstId,
    op: BinaryOp,
    a: Operand,
    b: Operand,
    flags: Flags,
}

fn bin_op(
    c: &Combiner,
    operand: Operand,
) -> Option<Bin> {
    let q = query(c, None);
    let (inst, instruction) = q.instruction_of(operand)?;
    let Opcode::Binary(op) = instruction.opcode else { return None };
    Some(Bin { inst, op, a: instruction.operands[0], b: instruction.operands[1], flags: instruction.flags })
}

/// `hasOneUse` of the operation's result.
fn has_one_use(
    c: &Combiner,
    bin: &Bin,
) -> bool {
    c.function.instruction(bin.inst).result.is_some_and(|result| c.function.users(result).has_one_use())
}

fn constant(
    c: &Combiner,
    operand: Operand,
) -> Option<(u128, u32)> {
    query(c, None).int_constant(operand)
}

fn is_equality(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Eq | IntPredicate::Ne)
}

fn is_signed(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
    )
}

fn is_unsigned(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Ugt | IntPredicate::Uge | IntPredicate::Ult | IntPredicate::Ule
    )
}

/// `isLT() || isGE()`... `ICmpInst::isLT`: `slt` or `ult`.
fn is_lt(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Slt | IntPredicate::Ult)
}

fn is_ge(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Sge | IntPredicate::Uge)
}

fn is_gt(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Sgt | IntPredicate::Ugt)
}

fn is_le(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Sle | IntPredicate::Ule)
}

/// `CmpInst::getFlippedStrictnessPredicate`.
fn flipped_strictness(pred: IntPredicate) -> IntPredicate {
    match pred {
        IntPredicate::Sgt => IntPredicate::Sge,
        IntPredicate::Sge => IntPredicate::Sgt,
        IntPredicate::Slt => IntPredicate::Sle,
        IntPredicate::Sle => IntPredicate::Slt,
        IntPredicate::Ugt => IntPredicate::Uge,
        IntPredicate::Uge => IntPredicate::Ugt,
        IntPredicate::Ult => IntPredicate::Ule,
        IntPredicate::Ule => IntPredicate::Ult,
        other => other,
    }
}

fn new_icmp(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    a: Operand,
    b: Operand,
) -> Option<Visit> {
    let i1 = c.context.types.int(1);
    Some(c.replace_with_new(inst, Opcode::ICmp(pred), i1, vec![a, b], Flags::default()))
}

/// What `hasNoWrapProblem` reads of a binary operator: its `nsw` and `nuw`
/// and whether the compare cannot see a wrap.
struct Wrap {
    nsw: bool,
    nuw: bool,
    none: bool,
}

fn wrap_of(
    bin: &Bin,
    pred: IntPredicate,
) -> Wrap {
    match bin.op {
        // OverflowingBinaryOperator.
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl => {
            let nuw = bin.flags.contains(Flags::NUW);
            let nsw = bin.flags.contains(Flags::NSW);
            Wrap { nsw, nuw, none: is_equality(pred) || (is_unsigned(pred) && nuw) || (is_signed(pred) && nsw) }
        }
        // `m_AddLike` matches only `or disjoint`: an add with no wrap at all.
        BinaryOp::Or => Wrap { nsw: true, nuw: true, none: true },
        _ => Wrap { nsw: false, nuw: false, none: false },
    }
}

/// `m_AddLike(m_Value(A), m_Value(B))`.
fn add_like(
    c: &Combiner,
    operand: Operand,
) -> Option<(Operand, Operand)> {
    pattern::add_like(&query(c, None), operand, Flags::default())
}

/// `isMultipleOf` of `foldICmpBinOp`.
fn is_multiple_of(
    c: &Combiner,
    inst: InstId,
    x: Operand,
    divisor: u128,
) -> bool {
    if divisor == 1 {
        return true;
    }
    if !divisor.is_power_of_two() {
        return false;
    }
    known::masked_value_is_zero(&query(c, Some(inst)), x, divisor - 1, 0)
}

/// `ShareCommonDivisor`: `x` and `y` are both multiples of the constant `z`
/// (of its absolute value when `negative`).
fn share_common_divisor(
    c: &Combiner,
    inst: InstId,
    x: Operand,
    y: Operand,
    z: Operand,
    negative: bool,
) -> bool {
    let Some((offset, width)) = constant(c, z) else { return false };
    // Fast path for Z == 1/-1.
    if if negative { offset == mask(width) } else { offset == 1 } {
        return true;
    }
    let value = if negative { offset.wrapping_neg() & mask(width) } else { offset };
    if signed(value, width) <= 0 {
        return false;
    }
    is_multiple_of(c, inst, x, value) && is_multiple_of(c, inst, y, value)
}

/// `foldICmpBinOp`.
pub fn fold_icmp_bin_op(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    // Special logic for binary operators.
    let bo0 = bin_op(c, op0);
    let bo1 = bin_op(c, op1);
    if bo0.is_none() && bo1.is_none() {
        return None;
    }
    // OWED: foldICmpXNegX.

    // (X urem Y) == X --> X u< Y
    // (X urem Y) != X --> X u>= Y
    if is_equality(pred) {
        let urem = |bin: &Option<Bin>, other: Operand| {
            bin.as_ref().filter(|bin| bin.op == BinaryOp::URem && bin.a == other).map(|bin| (bin.a, bin.b))
        };
        if let Some((dividend, divisor)) = urem(&bo0, op1).or_else(|| urem(&bo1, op0)) {
            let new = if pred == IntPredicate::Eq { IntPredicate::Ult } else { IntPredicate::Uge };
            return new_icmp(c, inst, new, dividend, divisor);
        }
    }

    // Convert add-with-unsigned-overflow comparisons into a 'not' with compare.
    // (Op1 + X) u</u>= Op1 --> ~Op1 u</u>= X
    if let Some(add) = bo0.as_ref().filter(|bin| bin.op == BinaryOp::Add && has_one_use(c, bin))
        && matches!(pred, IntPredicate::Ult | IntPredicate::Uge)
        && let Some(x) = other_of(add, op1)
    {
        let not = c.create_not(inst, op1);
        return new_icmp(c, inst, pred, not, x);
    }
    // Op0 u>/u<= (Op0 + X) --> X u>/u<= ~Op0
    if let Some(add) = bo1.as_ref().filter(|bin| bin.op == BinaryOp::Add && has_one_use(c, bin))
        && matches!(pred, IntPredicate::Ugt | IntPredicate::Ule)
        && let Some(x) = other_of(add, op0)
    {
        let not = c.create_not(inst, op0);
        return new_icmp(c, inst, pred, x, not);
    }

    // (Op1 + X) + C u</u>= Op1 --> ~C - X u</u>= Op1
    if let Some(outer) = bo0.as_ref().filter(|bin| bin.op == BinaryOp::Add && has_one_use(c, bin))
        && matches!(pred, IntPredicate::Ult | IntPredicate::Uge)
        && let Some((bits, width)) = constant(c, outer.b)
        && let Some(inner) = bin_op(c, outer.a).filter(|bin| bin.op == BinaryOp::Add)
        && let Some(x) = other_of(&inner, op1)
    {
        let not_c = c.int(c.type_of(outer.b), !bits & mask(width));
        let sub = c.create_sub(inst, not_c, x);
        return new_icmp(c, inst, pred, sub, op1);
    }
    // Op0 u>/u<= (Op0 + X) + C --> Op0 u>/u<= ~C - X
    if let Some(outer) = bo1.as_ref().filter(|bin| bin.op == BinaryOp::Add && has_one_use(c, bin))
        && matches!(pred, IntPredicate::Ugt | IntPredicate::Ule)
        && let Some((bits, width)) = constant(c, outer.b)
        && let Some(inner) = bin_op(c, outer.a).filter(|bin| bin.op == BinaryOp::Add)
        && let Some(x) = other_of(&inner, op0)
    {
        let not_c = c.int(c.type_of(outer.b), !bits & mask(width));
        let sub = c.create_sub(inst, not_c, x);
        return new_icmp(c, inst, pred, op0, sub);
    }

    // (icmp eq/ne (X, -P2), INT_MIN) -> (icmp slt/sge X, INT_MIN + P2)
    if is_equality(pred)
        && let Some(and) = bo0.as_ref().filter(|bin| bin.op == BinaryOp::And)
        && let Some((sign, width)) = constant(c, op1)
        && sign == 1u128 << (width - 1)
        && let Some((bits, _)) = constant(c, and.b)
        && ((bits.wrapping_neg() & mask(width)).is_power_of_two() || bits == 0)
    {
        // Will constant fold.
        let new_c = c.int(c.type_of(op1), sign.wrapping_sub(bits) & mask(width));
        let new = if pred == IntPredicate::Eq { IntPredicate::Slt } else { IntPredicate::Sge };
        return new_icmp(c, inst, new, and.a, new_c);
    }

    // Similar to above: an unsigned overflow comparison may use offset + mask:
    // ((Op1 + C) & C) u<  Op1 --> Op1 != 0
    // ((Op1 + C) & C) u>= Op1 --> Op1 == 0
    // Op0 u>  ((Op0 + C) & C) --> Op0 != 0
    // Op0 u<= ((Op0 + C) & C) --> Op0 == 0
    if matches!(pred, IntPredicate::Ult | IntPredicate::Uge) && is_offset_mask_of(c, bo0.as_ref(), op1) {
        let new = if pred == IntPredicate::Ult { IntPredicate::Ne } else { IntPredicate::Eq };
        let zero = c.int(c.type_of(op1), 0);
        return new_icmp(c, inst, new, op1, zero);
    }
    if matches!(pred, IntPredicate::Ugt | IntPredicate::Ule) && is_offset_mask_of(c, bo1.as_ref(), op0) {
        let new = if pred == IntPredicate::Ugt { IntPredicate::Ne } else { IntPredicate::Eq };
        let zero = c.int(c.type_of(op0), 0);
        return new_icmp(c, inst, new, op0, zero);
    }

    let wrap0 = bo0.as_ref().map(|bin| wrap_of(bin, pred));
    let wrap1 = bo1.as_ref().map(|bin| wrap_of(bin, pred));
    let no_op0_wrap_problem = wrap0.as_ref().is_some_and(|wrap| wrap.none);
    let no_op1_wrap_problem = wrap1.as_ref().is_some_and(|wrap| wrap.none);
    let (op0_nsw, op0_nuw) = wrap0.as_ref().map_or((false, false), |wrap| (wrap.nsw, wrap.nuw));
    let (op1_nsw, op1_nuw) = wrap1.as_ref().map_or((false, false), |wrap| (wrap.nsw, wrap.nuw));

    // Analyze the case when either Op0 or Op1 is an add instruction.
    // Op0 = A + B (or A and B are null); Op1 = C + D (or C and D are null).
    let (a, b) = if bo0.is_some() { add_like(c, op0).unzip() } else { (None, None) };
    let (cc, d) = if bo1.is_some() { add_like(c, op1).unzip() } else { (None, None) };

    // icmp (A+B), A -> icmp B, 0 for equalities or if there is no overflow.
    // icmp (A+B), B -> icmp A, 0 for equalities or if there is no overflow.
    if (a == Some(op1) || b == Some(op1)) && no_op0_wrap_problem {
        let rest = if a == Some(op1) { b } else { a }.expect("a part of the sum");
        let zero = c.int(c.type_of(op1), 0);
        return new_icmp(c, inst, pred, rest, zero);
    }

    // icmp C, (C+D) -> icmp 0, D for equalities or if there is no overflow.
    // icmp D, (C+D) -> icmp 0, C for equalities or if there is no overflow.
    if (cc == Some(op0) || d == Some(op0)) && no_op1_wrap_problem {
        let rest = if cc == Some(op0) { d } else { cc }.expect("a part of the sum");
        let zero = c.int(c.type_of(op0), 0);
        return new_icmp(c, inst, pred, zero, rest);
    }

    // icmp (A+B), (A+D) -> icmp B, D for equalities or if there is no overflow.
    if let (Some(a), Some(b), Some(cc), Some(d)) = (a, b, cc, d)
        && (a == cc || a == d || b == cc || b == d)
        && no_op0_wrap_problem
        && no_op1_wrap_problem
    {
        // Determine Y and Z in the form icmp (X+Y), (X+Z).
        let (y, z) = if a == cc {
            // C + B == C + D  ->  B == D
            (b, d)
        } else if a == d {
            // D + B == C + D  ->  B == C
            (b, cc)
        } else if b == cc {
            // A + C == C + D  ->  A == D
            (a, d)
        } else {
            // A + D == C + D  ->  A == C
            (a, cc)
        };
        return new_icmp(c, inst, pred, y, z);
    }

    if !is_equality(pred) {
        // The subtraction-related identities (A -nuw B) need A >=u B; sub nuw
        // is canonicalized to add of ones, so those are skipped as in LLVM.
        //
        // icmp slt (A + -1), Op1 -> icmp sle A, Op1
        // icmp sge (A + -1), Op1 -> icmp sgt A, Op1
        // icmp sle (A + 1), Op1 -> icmp slt A, Op1
        // icmp sgt (A + 1), Op1 -> icmp sge A, Op1
        // icmp ule (A + 1), Op0 -> icmp ult A, Op1
        // icmp ugt (A + 1), Op0 -> icmp uge A, Op1
        let negative = is_lt(pred) || is_ge(pred);
        let add_or_signed_pred = !negative || is_signed(pred);
        if let (Some(a), Some(b)) = (a, b)
            && no_op0_wrap_problem
            && add_or_signed_pred
            && share_common_divisor(c, inst, a, op1, b, negative)
        {
            return new_icmp(c, inst, flipped_strictness(pred), a, op1);
        }

        // icmp sgt Op0, (C + -1) -> icmp sge Op0, C
        // icmp sle Op0, (C + -1) -> icmp slt Op0, C
        // icmp sge Op0, (C + 1) -> icmp sgt Op0, C
        // icmp slt Op0, (C + 1) -> icmp sle Op0, C
        // icmp uge Op0, (C + 1) -> icmp ugt Op0, C
        // icmp ult Op0, (C + 1) -> icmp ule Op0, C
        if let (Some(cc), Some(d)) = (cc, d)
            && no_op1_wrap_problem
            && share_common_divisor(c, inst, op0, cc, d, is_gt(pred) || is_le(pred))
        {
            return new_icmp(c, inst, flipped_strictness(pred), op0, cc);
        }
    }

    // if C1 has greater magnitude than C2:
    //  icmp (A + C1), (C + C2) -> icmp (A + C3), C
    //  s.t. C3 = C1 - C2
    //
    // if C2 has greater magnitude than C1:
    //  icmp (A + C1), (C + C2) -> icmp A, (C + C3)
    //  s.t. C3 = C2 - C1
    if let (Some(a), Some(b), Some(cc), Some(d)) = (a, b, cc, d)
        && no_op0_wrap_problem
        && no_op1_wrap_problem
        && (bo0.as_ref().is_some_and(|bin| has_one_use(c, bin)) || bo1.as_ref().is_some_and(|bin| has_one_use(c, bin)))
        && !is_unsigned(pred)
    {
        if let (Some((ap1, width)), Some((ap2, _))) = (constant(c, b), constant(c, d))
            && (signed(ap1, width) < 0) == (signed(ap2, width) < 0)
        {
            let abs = |value: u128| if signed(value, width) < 0 { value.wrapping_neg() & mask(width) } else { value };
            let ty = c.type_of(op0);
            if abs(ap1) >= abs(ap2) {
                let diff = ap1.wrapping_sub(ap2) & mask(width);
                let c3 = c.int(ty, diff);
                let flags = flags_of(op0_nuw && diff <= ap1, op0_nsw);
                let new_add = c.create_binop(inst, BinaryOp::Add, a, c3, flags);
                return new_icmp(c, inst, pred, new_add, cc);
            }
            let diff = ap2.wrapping_sub(ap1) & mask(width);
            let c3 = c.int(ty, diff);
            let flags = flags_of(op1_nuw && diff <= ap2, op1_nsw);
            let new_add = c.create_binop(inst, BinaryOp::Add, cc, c3, flags);
            return new_icmp(c, inst, pred, a, new_add);
        }
        if is_equality(pred)
            && let (Some((cst1, width)), Some((cst2, _))) = (constant(c, b), constant(c, d))
        {
            let diff = c.int(c.type_of(op0), cst2.wrapping_sub(cst1) & mask(width));
            let new_add = c.create_add(inst, cc, diff);
            return new_icmp(c, inst, pred, a, new_add);
        }
    }

    // Analyze the case when either Op0 or Op1 is a sub instruction.
    // Op0 = A - B (or A and B are null); Op1 = C - D (or C and D are null).
    let (a, b) = match bo0.as_ref().filter(|bin| bin.op == BinaryOp::Sub) {
        Some(sub) => (Some(sub.a), Some(sub.b)),
        None => (None, None),
    };
    let (cc, d) = match bo1.as_ref().filter(|bin| bin.op == BinaryOp::Sub) {
        Some(sub) => (Some(sub.a), Some(sub.b)),
        None => (None, None),
    };

    // icmp (A-B), A -> icmp 0, B for equalities or if there is no overflow.
    if a == Some(op1) && no_op0_wrap_problem {
        let zero = c.int(c.type_of(op1), 0);
        return new_icmp(c, inst, pred, zero, b.expect("a difference"));
    }
    // icmp C, (C-D) -> icmp D, 0 for equalities or if there is no overflow.
    if cc == Some(op0) && no_op1_wrap_problem {
        let zero = c.int(c.type_of(op0), 0);
        return new_icmp(c, inst, pred, d.expect("a difference"), zero);
    }

    // Convert sub-with-unsigned-overflow comparisons into a comparison of args.
    // (A - B) u>/u<= A --> B u>/u<= A
    if a == Some(op1) && matches!(pred, IntPredicate::Ugt | IntPredicate::Ule) {
        return new_icmp(c, inst, pred, b.expect("a difference"), op1);
    }
    // C u</u>= (C - D) --> C u</u>= D
    if cc == Some(op0) && matches!(pred, IntPredicate::Ult | IntPredicate::Uge) {
        return new_icmp(c, inst, pred, op0, d.expect("a difference"));
    }
    // (A - B) u>=/u< A --> B u>/u<= A  iff B != 0
    if a == Some(op1)
        && matches!(pred, IntPredicate::Uge | IntPredicate::Ult)
        && nonzero::is_known_non_zero(&query(c, Some(inst)), b.expect("a difference"), 0)
    {
        return new_icmp(c, inst, flipped_strictness(pred), b.expect("a difference"), op1);
    }
    // C u<=/u> (C - D) --> C u</u>= D  iff B != 0
    if cc == Some(op0)
        && matches!(pred, IntPredicate::Ule | IntPredicate::Ugt)
        && nonzero::is_known_non_zero(&query(c, Some(inst)), d.expect("a difference"), 0)
    {
        return new_icmp(c, inst, flipped_strictness(pred), op0, d.expect("a difference"));
    }

    // icmp (A-B), (C-B) -> icmp A, C for equalities or if there is no overflow.
    if let (Some(a), Some(b), Some(cc), Some(d)) = (a, b, cc, d)
        && b == d
        && no_op0_wrap_problem
        && no_op1_wrap_problem
    {
        return new_icmp(c, inst, pred, a, cc);
    }

    // icmp (A-B), (A-D) -> icmp D, B for equalities or if there is no overflow.
    if let (Some(a), Some(b), Some(cc), Some(d)) = (a, b, cc, d)
        && a == cc
        && no_op0_wrap_problem
        && no_op1_wrap_problem
    {
        return new_icmp(c, inst, pred, d, b);
    }

    // icmp (0-X) < cst --> x > -cst
    if no_op0_wrap_problem && is_signed(pred) {
        let negated = pattern::neg(&query(c, None), op0).map(|(_, x, _)| x);
        if let Some(x) = negated
            && let Some((bits, width)) = constant(c, op1)
            && bits != 1u128 << (width - 1)
        {
            let negative = c.int(c.type_of(op1), bits.wrapping_neg() & mask(width));
            return new_icmp(c, inst, pred.swapped(), x, negative);
        }
    }

    // OWED: foldICmpXorXX, foldICmpOrXX, the shared multiplier, the `srem`
    // against its divisor, the operations of one opcode with a shared right
    // operand, foldICmpAndXX, foldMultiplicationOverflowCheck,
    // foldICmpWithTruncSignExtendedVal and
    // foldShiftIntoShiftInAnotherHandOfAndInICmp.
    None
}

/// The other operand of the `add` `bin` when one of them is `wanted`
/// (`m_c_Add(m_Specific(wanted), m_Value(X))`).
fn other_of(
    bin: &Bin,
    wanted: Operand,
) -> Option<Operand> {
    if bin.a == wanted {
        Some(bin.b)
    } else if bin.b == wanted {
        Some(bin.a)
    } else {
        None
    }
}

/// `match(Op, m_And(m_BinOp(BO), m_LowBitMask(C))) && match(BO,
/// m_Add(m_Specific(other), C))`.
fn is_offset_mask_of(
    c: &Combiner,
    and: Option<&Bin>,
    other: Operand,
) -> bool {
    let Some(and) = and.filter(|bin| bin.op == BinaryOp::And) else { return false };
    let Some((mask_bits, _)) = constant(c, and.b) else { return false };
    // m_LowBitMask: a non-zero run of ones from bit 0.
    if mask_bits == 0 || mask_bits & mask_bits.wrapping_add(1) != 0 {
        return false;
    }
    let Some(add) = bin_op(c, and.a).filter(|bin| bin.op == BinaryOp::Add) else { return false };
    add.a == other && constant(c, add.b).is_some_and(|(bits, _)| bits == mask_bits)
}

fn flags_of(
    nuw: bool,
    nsw: bool,
) -> Flags {
    let mut flags = Flags::default();
    if nuw {
        flags = flags.union(Flags::NUW);
    }
    if nsw {
        flags = flags.union(Flags::NSW);
    }
    flags
}

#[cfg(test)]
#[path = "combine_icmp_binop_tests.rs"]
mod tests;
