//! InstCombine's `foldICmpBinOp`, the add and subtract arms
//! ([`InstCombineCompares.cpp:5301`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L5301)
//! at d1106deb71cc).
//!
//! MIRROR: the order of the steps and each condition, up to the `Xor`/`Or`
//! folds.
//!
//! OWED, in place as `// OWED:` comments: the equality forms of
//! `foldICmpOrXX`, and every arm after `srem` (the shared second operand,
//! `udiv`, `shl`, `lshr`, `and`...).
//!
//! DEVIATE 1: `ShareCommonDivisor` takes only its fast path (the offset is 1,
//! or -1 for the negative forms); the general case needs `isMultipleOf`, which
//! MIR's value tracking does not have. Exit: `isMultipleOf`.
//!
//! DEVIATE 2: no vectors; the constants of the equality fold are integers.
//!
//! DEVIATE 3: the shared multiplier asks `simplifyICmpInst` without the
//! instruction as its context.
//!
//! # How it works
//! With `Op0 = A + B` and `Op1 = C + D` (an `or disjoint` is an add), a
//! relational compare that sees one side offset by one steps the strictness
//! instead and drops the offset: `A + 1 s> Y` is `A s>= Y`, `X s> C + -1` is
//! `X s>= C`. The offset must not wrap in the compare's signedness.
//!
//! # Example
//! The sift loop of x_heapsort kept `or disjoint %x, 1` live for its guard:
//!
//! ```text
//! %l = or disjoint i32 %x, 1   ; %c = icmp sge i32 %x, %n
//! %c = icmp sgt i32 %l, %n
//! ```

use llrm_mir::context::{mask, signed};
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::simplify::simplify_icmp_operands;
use llrm_mir::valuetracking::{known, nonzero, pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{has_one_use, query};
use crate::constraintelim::strict_predicate;

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

/// `CmpInst::getFlippedStrictnessPredicate`.
fn flipped_strictness(pred: IntPredicate) -> IntPredicate {
    match pred {
        IntPredicate::Ugt => IntPredicate::Uge,
        IntPredicate::Uge => IntPredicate::Ugt,
        IntPredicate::Ult => IntPredicate::Ule,
        IntPredicate::Ule => IntPredicate::Ult,
        IntPredicate::Sgt => IntPredicate::Sge,
        IntPredicate::Sge => IntPredicate::Sgt,
        IntPredicate::Slt => IntPredicate::Sle,
        IntPredicate::Sle => IntPredicate::Slt,
        other => other,
    }
}

/// `ICmpInst::isGT`, `isLE`: the predicates of the "greater" and "at most"
/// kind.
fn is_gt_or_le(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Ugt | IntPredicate::Sgt | IntPredicate::Ule | IntPredicate::Sle
    )
}

/// `ICmpInst::isLT`, `isGE`.
fn is_lt_or_ge(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Ult | IntPredicate::Slt | IntPredicate::Uge | IntPredicate::Sge
    )
}

/// A binary operator `op`: its opcode, operands and flags
/// (`dyn_cast<BinaryOperator>`).
fn binary_operator(
    c: &Combiner,
    op: Operand,
) -> Option<(BinaryOp, Operand, Operand, Flags)> {
    let (_, instruction) = query(c, None).instruction_of(op)?;
    match instruction.opcode {
        Opcode::Binary(bin) => Some((bin, instruction.operands[0], instruction.operands[1], instruction.flags)),
        _ => None,
    }
}

/// One side of the compare: the operator, its `m_AddLike` parts, whether its
/// wrap flags cannot hurt this predicate (`hasNoWrapProblem`), and its flags.
struct Side {
    op: Option<BinaryOp>,
    a: Option<Operand>,
    b: Option<Operand>,
    no_wrap_problem: bool,
    has_nuw: bool,
    has_nsw: bool,
    one_use: bool,
}

fn analyse(
    c: &Combiner,
    inst: InstId,
    pred: IntPredicate,
    operand: Operand,
) -> Side {
    let mut side =
        Side { op: None, a: None, b: None, no_wrap_problem: false, has_nuw: false, has_nsw: false, one_use: false };
    let Some((op, _, _, flags)) = binary_operator(c, operand) else { return side };
    side.op = Some(op);
    side.one_use = has_one_use(c, operand);
    if let Some((a, b)) = pattern::add_like(&query(c, Some(inst)), operand, Flags::default()) {
        side.a = Some(a);
        side.b = Some(b);
    }
    // `isa<OverflowingBinaryOperator>`: add, sub, mul and shl.
    if matches!(
        op,
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl
    ) {
        side.has_nuw = flags.contains(Flags::NUW);
        side.has_nsw = flags.contains(Flags::NSW);
        side.no_wrap_problem =
            is_equality(pred) || (is_unsigned(pred) && side.has_nuw) || (is_signed(pred) && side.has_nsw);
    } else if op == BinaryOp::Or {
        // The invariant is that the add-like matches only an `or disjoint`,
        // which is an add nuw nsw.
        side.has_nuw = true;
        side.has_nsw = true;
        side.no_wrap_problem = true;
    }
    side
}

/// `ShareCommonDivisor`, fast path (DEVIATE 1): the offset is 1, or -1 when
/// `negative`.
fn share_common_divisor(
    c: &Combiner,
    z: Operand,
    negative: bool,
) -> bool {
    let Some((bits, width)) = query(c, None).int_constant(z) else { return false };
    if negative { bits == mask(width) } else { bits == 1 }
}

/// `foldICmpBinOp`, up to the folds that are OWED.
pub fn fold_icmp_bin_op(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let bo0 = binary_operator(c, op0);
    let bo1 = binary_operator(c, op1);
    if bo0.is_none() && bo1.is_none() {
        return None;
    }
    let ty = c.type_of(op0);
    if let Some(r) = fold_icmp_x_neg_x(c, inst, pred, op0, op1) {
        return Some(r);
    }

    // (X urem Y) == X --> X u< Y
    // (X urem Y) != X --> X u>= Y
    if is_equality(pred) {
        let urem_of = |c: &Combiner, urem: Operand, other: Operand| {
            binary_operator(c, urem).filter(|&(op, x, _, _)| op == BinaryOp::URem && x == other)
        };
        let found = urem_of(c, op0, op1).or_else(|| urem_of(c, op1, op0));
        if let Some((_, dividend, divisor, _)) = found {
            let new = if pred == IntPredicate::Eq { IntPredicate::Ult } else { IntPredicate::Uge };
            return Some(new_icmp(c, inst, new, dividend, divisor));
        }
    }

    // Convert add-with-unsigned-overflow comparisons into a 'not' with compare.
    // (Op1 + X) u</u>= Op1 --> ~Op1 u</u>= X
    if matches!(pred, IntPredicate::Ult | IntPredicate::Uge)
        && let Some(x) = one_use_commutative_add(c, op0, op1)
    {
        let not = c.create_binop(inst, BinaryOp::Xor, op1, c.all_ones(ty), Flags::default());
        return Some(new_icmp(c, inst, pred, not, x));
    }
    // Op0 u>/u<= (Op0 + X) --> X u>/u<= ~Op0
    if matches!(pred, IntPredicate::Ugt | IntPredicate::Ule)
        && let Some(x) = one_use_commutative_add(c, op1, op0)
    {
        let not = c.create_binop(inst, BinaryOp::Xor, op0, c.all_ones(ty), Flags::default());
        return Some(new_icmp(c, inst, pred, x, not));
    }
    // (Op1 + X) + C u</u>= Op1 --> ~C - X u</u>= Op1
    // Op0 u>/u<= (Op0 + X) + C --> Op0 u>/u<= ~C - X
    if let Some(r) = fold_add_add_constant(c, inst, pred, op0, op1) {
        return Some(r);
    }
    // (icmp eq/ne (X & -P2), INT_MIN) -> (icmp slt/sge X, INT_MIN + P2)
    if let Some(r) = bo0.and_then(|(op, x, mask_op, _)| fold_sign_mask_equality(c, inst, pred, op, x, mask_op, op1, ty))
    {
        return Some(r);
    }
    // ((Op1 + C) & C) u< Op1 --> Op1 != 0 and the three like it.
    if let Some(r) = fold_low_bit_mask_overflow(c, inst, pred, op0, op1, ty) {
        return Some(r);
    }

    let side0 = analyse(c, inst, pred, op0);
    let side1 = analyse(c, inst, pred, op1);
    let (a, b, cc, d) = (side0.a, side0.b, side1.a, side1.b);

    // icmp (A+B), A -> icmp B, 0 for equalities or if there is no overflow.
    // icmp (A+B), B -> icmp A, 0 for equalities or if there is no overflow.
    if side0.no_wrap_problem && (a == Some(op1) || b == Some(op1)) {
        let other = if a == Some(op1) { b } else { a }.expect("an add-like pair");
        let zero = c.int(c.type_of(op1), 0);
        return Some(new_icmp(c, inst, pred, other, zero));
    }
    // icmp C, (C+D) -> icmp 0, D for equalities or if there is no overflow.
    // icmp D, (C+D) -> icmp 0, C for equalities or if there is no overflow.
    if side1.no_wrap_problem && (cc == Some(op0) || d == Some(op0)) {
        let other = if cc == Some(op0) { d } else { cc }.expect("an add-like pair");
        let zero = c.int(c.type_of(op0), 0);
        return Some(new_icmp(c, inst, pred, zero, other));
    }
    // icmp (A+B), (A+D) -> icmp B, D for equalities or if there is no overflow.
    if let (Some(a), Some(b), Some(cc), Some(d)) = (a, b, cc, d)
        && (a == cc || a == d || b == cc || b == d)
        && side0.no_wrap_problem
        && side1.no_wrap_problem
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

    if !is_equality(pred) {
        // icmp sgt (A + 1), Op1 -> icmp sge A, Op1 and the like.
        let negative = is_lt_or_ge(pred);
        let add_or_signed = !negative || is_signed(pred);
        if let (Some(a), Some(b)) = (a, b)
            && side0.no_wrap_problem
            && add_or_signed
            && share_common_divisor(c, b, negative)
        {
            return Some(new_icmp(c, inst, flipped_strictness(pred), a, op1));
        }
        // icmp sgt Op0, (C + -1) -> icmp sge Op0, C and the like.
        if let (Some(cc), Some(d)) = (cc, d)
            && side1.no_wrap_problem
            && share_common_divisor(c, d, is_gt_or_le(pred))
        {
            return Some(new_icmp(c, inst, flipped_strictness(pred), op0, cc));
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
        && side0.no_wrap_problem
        && side1.no_wrap_problem
        && (side0.one_use || side1.one_use)
        && !is_unsigned(pred)
    {
        let ty = c.type_of(op0);
        if let (Some((ap1, width)), Some((ap2, _))) = (query(c, None).int_constant(b), query(c, None).int_constant(d)) {
            let (s1, s2) = (signed(ap1, width), signed(ap2, width));
            if (s1 < 0) == (s2 < 0) {
                if s1.unsigned_abs() >= s2.unsigned_abs() {
                    let diff = ap1.wrapping_sub(ap2) & mask(width);
                    let c3 = c.int(ty, diff);
                    let mut flags = Flags::default();
                    if side0.has_nuw && diff <= ap1 {
                        flags = flags.union(Flags::NUW);
                    }
                    if side0.has_nsw {
                        flags = flags.union(Flags::NSW);
                    }
                    let new_add = c.create_binop(inst, BinaryOp::Add, a, c3, flags);
                    return Some(new_icmp(c, inst, pred, new_add, cc));
                }
                let diff = ap2.wrapping_sub(ap1) & mask(width);
                let c3 = c.int(ty, diff);
                let mut flags = Flags::default();
                if side1.has_nuw && diff <= ap2 {
                    flags = flags.union(Flags::NUW);
                }
                if side1.has_nsw {
                    flags = flags.union(Flags::NSW);
                }
                let new_add = c.create_binop(inst, BinaryOp::Add, cc, c3, flags);
                return Some(new_icmp(c, inst, pred, a, new_add));
            }
        }
        // icmp eq/ne (A + Cst1), (C + Cst2) -> icmp A, (C + (Cst2 - Cst1))
        if is_equality(pred)
            && let (Some((b_bits, width)), Some((d_bits, _))) =
                (query(c, None).int_constant(b), query(c, None).int_constant(d))
        {
            let diff = c.int(ty, d_bits.wrapping_sub(b_bits) & mask(width));
            let new_add = c.create_binop(inst, BinaryOp::Add, cc, diff, Flags::default());
            return Some(new_icmp(c, inst, pred, a, new_add));
        }
    }

    // Analyze the case when either Op0 or Op1 is a sub instruction.
    // Op0 = A - B (or A and B are null); Op1 = C - D (or C and D are null).
    let sub_of = |bo: &Option<(BinaryOp, Operand, Operand, Flags)>| {
        bo.filter(|&(op, _, _, _)| op == BinaryOp::Sub).map(|(_, x, y, _)| (x, y))
    };
    let (a, b) = sub_of(&bo0).unzip();
    let (cc, d) = sub_of(&bo1).unzip();

    // icmp (A-B), A -> icmp 0, B for equalities or if there is no overflow.
    if a == Some(op1) && side0.no_wrap_problem {
        let zero = c.int(c.type_of(op1), 0);
        return Some(new_icmp(c, inst, pred, zero, b.expect("a sub")));
    }
    // icmp C, (C-D) -> icmp D, 0 for equalities or if there is no overflow.
    if cc == Some(op0) && side1.no_wrap_problem {
        let zero = c.int(c.type_of(op0), 0);
        return Some(new_icmp(c, inst, pred, d.expect("a sub"), zero));
    }
    // (A - B) u>/u<= A --> B u>/u<= A
    if a == Some(op1) && matches!(pred, IntPredicate::Ugt | IntPredicate::Ule) {
        return Some(new_icmp(c, inst, pred, b.expect("a sub"), op1));
    }
    // C u</u>= (C - D) --> C u</u>= D
    if cc == Some(op0) && matches!(pred, IntPredicate::Ult | IntPredicate::Uge) {
        return Some(new_icmp(c, inst, pred, op0, d.expect("a sub")));
    }
    // (A - B) u>=/u< A --> B u>/u<= A  iff B != 0
    if a == Some(op1)
        && matches!(pred, IntPredicate::Uge | IntPredicate::Ult)
        && nonzero::is_known_non_zero(&query(c, Some(inst)), b.expect("a sub"), 0)
    {
        return Some(new_icmp(c, inst, flipped_strictness(pred), b.expect("a sub"), op1));
    }
    // C u<=/u> (C - D) --> C u</u>= D  iff D != 0
    if cc == Some(op0)
        && matches!(pred, IntPredicate::Ule | IntPredicate::Ugt)
        && nonzero::is_known_non_zero(&query(c, Some(inst)), d.expect("a sub"), 0)
    {
        return Some(new_icmp(c, inst, flipped_strictness(pred), op0, d.expect("a sub")));
    }
    // icmp (A-B), (C-B) -> icmp A, C for equalities or if there is no overflow.
    if let (Some(a), Some(b), Some(cc), Some(d)) = (a, b, cc, d)
        && b == d
        && side0.no_wrap_problem
        && side1.no_wrap_problem
    {
        return Some(new_icmp(c, inst, pred, a, cc));
    }
    // icmp (A-B), (A-D) -> icmp D, B for equalities or if there is no overflow.
    if let (Some(a), Some(b), Some(cc), Some(d)) = (a, b, cc, d)
        && a == cc
        && side0.no_wrap_problem
        && side1.no_wrap_problem
    {
        return Some(new_icmp(c, inst, pred, d, b));
    }
    // icmp (0-X) < cst --> x > -cst
    if side0.no_wrap_problem
        && is_signed(pred)
        && side0.op == Some(BinaryOp::Sub)
        && let Some((_, zero, x, _)) = binary_operator(c, op0)
        && const_bits(c, zero) == Some(0)
        && let Some((bits, width)) = query(c, None).int_constant(op1)
        && bits != 1 << (width - 1)
    {
        let negated = c.int(ty, 0u128.wrapping_sub(bits));
        return Some(new_icmp(c, inst, swapped(pred), x, negated));
    }

    if let Some(r) = fold_icmp_xor_xx(c, inst, pred, op0, op1) {
        return Some(r);
    }
    if let Some(r) = fold_icmp_or_xx(c, inst, pred, op0, op1) {
        return Some(r);
    }
    if let Some(r) = fold_shared_multiplier(c, inst, pred, op0, op1, &side0, &side1) {
        return Some(r);
    }
    if let Some(r) = fold_srem_against_divisor(c, inst, pred, op0, op1) {
        return Some(r);
    }
    if let Some(r) = fold_same_opcode(c, inst, pred, op0, op1, &side0, &side1) {
        return Some(r);
    }
    if let Some(r) = fold_and_below_limit(c, inst, pred, op0, op1) {
        return Some(r);
    }
    if let Some(r) = fold_doubling(c, inst, pred, op0, op1) {
        return Some(r);
    }
    if let Some(r) = fold_multiplication_overflow_check(c, inst, pred, op0, op1) {
        return Some(r);
    }
    if let Some(r) = fold_icmp_and_xx(c, inst, pred, op0, op1) {
        return Some(r);
    }
    if let Some(r) = fold_icmp_with_trunc_sign_extended_val(c, inst, pred, op0, op1) {
        return Some(r);
    }
    // OWED: foldShiftIntoShiftInAnotherHandOfAndInICmp.
    None
}

/// `foldICmpWithTruncSignExtendedVal`: `((x << K) a>> K) == x` is a check for
/// a lossy signed truncation:
/// `(add x, (1 << (KeptBits-1))) u< (1 << KeptBits)`, `KeptBits = width - K`.
fn fold_icmp_with_trunc_sign_extended_val(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    // We are ok with 'shl' having multiple uses, but 'ashr' must be one-use.
    let extended = |c: &Combiner, ashr: Operand, x: Operand| {
        let (op, shl, amount, _) = binary_operator(c, ashr)?;
        let (inner, shifted, amount0, _) = binary_operator(c, shl)?;
        if op != BinaryOp::AShr || inner != BinaryOp::Shl || shifted != x || !has_one_use(c, ashr) {
            return None;
        }
        let (masked, width) = query(c, None).int_constant(amount)?;
        let (masked0, _) = query(c, None).int_constant(amount0)?;
        // The shift amounts must be equal.
        (masked == masked0 && masked != 0 && masked < u128::from(width)).then_some((masked as u32, width))
    };
    let (masked_bits, width) = extended(c, op0, op1).or_else(|| extended(c, op1, op0))?;
    let dst = match pred {
        IntPredicate::Eq => IntPredicate::Ult,
        IntPredicate::Ne => IntPredicate::Uge,
        _ => return None,
    };
    let x = if extended(c, op0, op1).is_some() { op1 } else { op0 };
    let ty = c.type_of(x);
    // KeptBits = bitwidth(%x) - MaskedBits
    let kept_bits = width - masked_bits;
    // ICmpCst = (1 << KeptBits), AddCst = (1 << (KeptBits-1))
    let icmp_cst = 1u128 << kept_bits;
    let add_cst = icmp_cst >> 1;
    let add_cst = c.int(ty, add_cst);
    let t0 = c.create_add(inst, x, add_cst);
    let limit = c.int(ty, icmp_cst);
    let t1 = c.create_icmp(inst, dst, t0, limit);
    Some(Visit::Replace(t1))
}

/// `foldMultiplicationOverflowCheck`: `(-1 u/ x) u< y` and
/// `((x * y) ?/ x) != y` are the overflow bit of `x * y`.
fn fold_multiplication_overflow_check(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (need_negation, signed, x, y, mul) = if !is_equality(pred) {
        // Look for: (-1 u/ x) u</u>= y
        let udiv_of_all_ones = |c: &Combiner, d: Operand| {
            let (op, ones, x, _) = binary_operator(c, d)?;
            let all_ones = query(c, None).int_constant(ones).is_some_and(|(bits, width)| bits == mask(width));
            (op == BinaryOp::UDiv && all_ones && has_one_use(c, d)).then_some(x)
        };
        let (pred, x, y) = if let Some(x) = udiv_of_all_ones(c, op0) {
            (pred, x, op1)
        } else {
            (swapped(pred), udiv_of_all_ones(c, op1)?, op0)
        };
        // Are we checking that overflow does not happen, or does happen?
        let need_negation = match pred {
            IntPredicate::Ult => false,
            IntPredicate::Uge => true,
            _ => return None,
        };
        (need_negation, false, x, y, None)
    } else {
        // Look for: ((x * y) / x) !=/== y
        let idiv_of_mul = |c: &Combiner, y: Operand, d: Operand| {
            let (op, product, x, _) = binary_operator(c, d)?;
            if !matches!(op, BinaryOp::UDiv | BinaryOp::SDiv) || !has_one_use(c, d) {
                return None;
            }
            // m_c_Mul(m_Deferred(Y), m_Value(X)), the divisor being X
            let (_, a, b, _) = binary_operator(c, product).filter(|&(op, ..)| op == BinaryOp::Mul)?;
            ((a == y && b == x) || (b == y && a == x)).then_some((op == BinaryOp::SDiv, x, product))
        };
        let (y, (signed, x, product)) = match idiv_of_mul(c, op0, op1) {
            Some(found) => (op0, found),
            None => (op1, idiv_of_mul(c, op1, op0)?),
        };
        (pred == IntPredicate::Eq, signed, x, y, Some(product))
    };
    let mul_inst = mul.and_then(|m| query(c, None).instruction_of(m).map(|(id, _)| id));
    let mul_had_other_uses = mul.is_some_and(|m| !has_one_use(c, m));
    // If the pattern included (x * y), the new instructions go right before
    // that original multiplication so that it can be replaced.
    let at = mul_inst.filter(|_| mul_had_other_uses).unwrap_or(inst);
    let pair = c.create_mul_with_overflow(at, signed, x, y);
    let i1 = c.context.types.int(1);
    if let (true, Some(mul_inst)) = (mul_had_other_uses, mul_inst) {
        // The multiplication was used elsewhere: its uses read the result.
        let ty = c.type_of(x);
        let value = c.create_extract_value(at, pair, 0, ty);
        c.replace_inst_with(mul_inst, value);
    }
    let mut result = c.create_extract_value(at, pair, 1, i1);
    if need_negation {
        result = c.create_not(at, result);
    }
    if let (true, Some(mul_inst)) = (mul_had_other_uses, mul_inst) {
        c.erase(mul_inst);
    }
    Some(Visit::Replace(result))
}

/// `foldICmpAndXX`: a compare of `X & A` with `X`.
fn fold_icmp_and_xx(
    c: &mut Combiner,
    inst: InstId,
    mut pred: IntPredicate,
    mut op0: Operand,
    mut op1: Operand,
) -> Option<Visit> {
    // Normalize and operand as operand 0.
    if commutative_with_specific(c, inst, op1, BinaryOp::And, op0).is_some() {
        std::mem::swap(&mut op0, &mut op1);
        pred = swapped(pred);
    }
    let a = commutative_with_specific(c, inst, op0, BinaryOp::And, op1)?;

    // (icmp (X & Y) u< X --> (X & Y) != X
    if pred == IntPredicate::Ult {
        return Some(new_icmp(c, inst, IntPredicate::Ne, op0, op1));
    }
    // (icmp (X & Y) u>= X --> (X & Y) == X
    if pred == IntPredicate::Uge {
        return Some(new_icmp(c, inst, IntPredicate::Eq, op0, op1));
    }
    if is_equality(pred) && has_one_use(c, op0) {
        let ty = c.type_of(op1);
        let mut consumes = false;
        // icmp (X & Y) eq/ne Y --> (X | ~Y) eq/ne -1 if Y is freely invertible
        // and Y is non-constant. If Y is constant the `X & C == C` form is
        // preferable so don't do this fold.
        if !is_constant(c, op1) {
            let invertible = !has_n_uses_or_more(c, op1, 3);
            if let Some(not_op1) = c.get_freely_inverted(inst, op1, invertible, true, &mut consumes) {
                let or = c.create_binop(inst, BinaryOp::Or, a, not_op1, Flags::default());
                let ones = c.all_ones(ty);
                return Some(new_icmp(c, inst, pred, or, ones));
            }
        }
        // icmp (X & Y) eq/ne Y --> (~X & Y) eq/ne 0 if X  is freely invertible.
        let a_inverts = has_one_use(c, a);
        if let Some(not_a) = c.get_freely_inverted(inst, a, a_inverts, true, &mut consumes) {
            let and = c.create_binop(inst, BinaryOp::And, op1, not_a, Flags::default());
            let zero = c.int(ty, 0);
            return Some(new_icmp(c, inst, pred, and, zero));
        }
    }
    if !is_signed(pred) {
        return None;
    }
    let q = query(c, Some(inst));
    let known_y = known::compute_known_bits(&q, a, 0);
    // (X & NegY) spred X --> (X & NegY) upred X
    if known_y.is_negative() {
        return Some(new_icmp(c, inst, flipped_signedness(pred), op0, op1));
    }
    if pred != IntPredicate::Sle && pred != IntPredicate::Sgt {
        return None;
    }
    let ty = c.type_of(op1);
    if known_y.is_non_negative() {
        // (X & PosY) s<= X --> X s>= 0
        // (X & PosY) s> X --> X s< 0
        let zero = c.int(ty, 0);
        return Some(new_icmp(c, inst, swapped(pred), op1, zero));
    }
    if known::is_known_negative(&q, op1, 0) {
        // (NegX & Y) s<= NegX --> Y s< 0
        // (NegX & Y) s> NegX --> Y s>= 0
        let zero = c.int(c.type_of(a), 0);
        return Some(new_icmp(c, inst, flipped_strictness(pred), a, zero));
    }
    None
}

/// `foldICmpXNegX`: a compare of `-X` with `X`.
fn fold_icmp_x_neg_x(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    // `m_Neg` is `sub 0, X`; of the two operands, the negation is the one
    // whose operand is the other (`m_c_ICmp`).
    let negation_of = |c: &Combiner, neg: Operand, x: Operand| {
        binary_operator(c, neg)
            .filter(|&(op, zero, v, _)| op == BinaryOp::Sub && v == x && const_bits(c, zero) == Some(0))
            .map(|(_, _, _, flags)| flags)
    };
    let (flags, x, pred, neg) = if let Some(flags) = negation_of(c, op0, op1) {
        (flags, op1, pred, op0)
    } else if let Some(flags) = negation_of(c, op1, op0) {
        (flags, op0, swapped(pred), op1)
    } else {
        return None;
    };
    let ty = c.type_of(x);
    if flags.contains(Flags::NSW) {
        let new = if is_signed(pred) {
            swapped(pred)
        } else if is_unsigned(pred) {
            flipped_signedness(pred)
        } else {
            pred
        };
        let zero = c.int(ty, 0);
        return Some(new_icmp(c, inst, new, x, zero));
    }
    // A value is not equal to its negation unless that value is 0 or
    // MinSignedValue, ie: a != -a --> (a & MaxSignedVal) != 0
    if is_equality(pred) && has_one_use(c, neg) {
        let width = query(c, None).int_constant(c.int(ty, 0)).map_or(0, |(_, w)| w);
        let max_signed = c.int(ty, (1u128 << (width - 1)) - 1);
        let and = c.create_binop(inst, BinaryOp::And, x, max_signed, Flags::default());
        let zero = c.int(ty, 0);
        return Some(new_icmp(c, inst, pred, and, zero));
    }
    None
}

/// `ICmpInst::getFlippedSignednessPredicate`.
fn flipped_signedness(pred: IntPredicate) -> IntPredicate {
    match pred {
        IntPredicate::Ugt => IntPredicate::Sgt,
        IntPredicate::Uge => IntPredicate::Sge,
        IntPredicate::Ult => IntPredicate::Slt,
        IntPredicate::Ule => IntPredicate::Sle,
        IntPredicate::Sgt => IntPredicate::Ugt,
        IntPredicate::Sge => IntPredicate::Uge,
        IntPredicate::Slt => IntPredicate::Ult,
        IntPredicate::Sle => IntPredicate::Ule,
        other => other,
    }
}

/// Two operators of one opcode with the same second operand, one of them
/// used only here.
fn fold_same_opcode(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
    side0: &Side,
    side1: &Side,
) -> Option<Visit> {
    let (opcode, a, y0, f0) = binary_operator(c, op0)?;
    let (opcode1, b, y1, f1) = binary_operator(c, op1)?;
    if opcode != opcode1 || !(side0.one_use || side1.one_use) || y0 != y1 {
        return None;
    }
    let ty = c.type_of(op0);
    let exact = f0.contains(Flags::EXACT) && f1.contains(Flags::EXACT);
    match opcode {
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Xor => {
            // a+x icmp eq/ne b+x --> a icmp b
            if is_equality(pred) {
                return Some(new_icmp(c, inst, pred, a, b));
            }
            let (bits, width) = query(c, None).int_constant(y0)?;
            // icmp u/s (a ^ signmask), (b ^ signmask) --> icmp s/u a, b
            if bits == 1 << (width - 1) {
                return Some(new_icmp(c, inst, flipped_signedness(pred), a, b));
            }
            // icmp u/s (a ^ maxsignval), (b ^ maxsignval) --> icmp s/u' a, b
            if opcode == BinaryOp::Xor && bits == (1 << (width - 1)) - 1 {
                return Some(new_icmp(c, inst, swapped(flipped_signedness(pred)), a, b));
            }
            None
        }
        BinaryOp::Mul => {
            if !is_equality(pred) {
                return None;
            }
            let (bits, width) = query(c, None).int_constant(y0)?;
            // icmp eq/ne (X * C), (Y * C) --> icmp (X & Mask), (Y & Mask)
            // Mask = -1 >> count-trailing-zeros(C).
            let tzs = bits.trailing_zeros();
            if bits == 0 || bits == 1 || tzs == 0 {
                return None;
            }
            let mask = c.int(ty, mask(width - tzs));
            let and1 = c.create_binop(inst, BinaryOp::And, a, mask, Flags::default());
            let and2 = c.create_binop(inst, BinaryOp::And, b, mask, Flags::default());
            Some(new_icmp(c, inst, pred, and1, and2))
        }
        BinaryOp::UDiv | BinaryOp::LShr => {
            if is_signed(pred) || !exact {
                return None;
            }
            Some(new_icmp(c, inst, pred, a, b))
        }
        BinaryOp::SDiv => {
            let non_negative = query(c, None).int_constant(y0).is_some_and(|(bits, width)| bits >> (width - 1) == 0);
            if !(is_equality(pred) || non_negative) || !exact {
                return None;
            }
            Some(new_icmp(c, inst, pred, a, b))
        }
        BinaryOp::AShr => exact.then(|| new_icmp(c, inst, pred, a, b)),
        BinaryOp::Shl => {
            let nuw = side0.has_nuw && side1.has_nuw;
            let nsw = side0.has_nsw && side1.has_nsw;
            if (!nuw && !nsw) || (!nsw && is_signed(pred)) {
                return None;
            }
            Some(new_icmp(c, inst, pred, a, b))
        }
        _ => None,
    }
}

/// Transform  A & (L - 1) `ult` L --> L != 0
fn fold_and_below_limit(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    if pred != IntPredicate::Ult {
        return None;
    }
    let (_, x, y, _) = binary_operator(c, op0).filter(|&(op, ..)| op == BinaryOp::And)?;
    let limit_minus_one = |c: &Combiner, v: Operand| {
        binary_operator(c, v).is_some_and(|(op, l, m, _)| {
            op == BinaryOp::Add
                && l == op1
                && query(c, None).int_constant(m).is_some_and(|(bits, width)| bits == mask(width))
        })
    };
    if !(limit_minus_one(c, y) || limit_minus_one(c, x)) {
        return None;
    }
    let zero = c.int(c.type_of(op1), 0);
    Some(new_icmp(c, inst, IntPredicate::Ne, op1, zero))
}

/// For unsigned predicates / eq / ne:
/// icmp pred (x << 1), x --> icmp getSignedPredicate(pred) x, 0
/// icmp pred x, (x << 1) --> icmp getSignedPredicate(pred) 0, x
fn fold_doubling(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    if is_signed(pred) {
        return None;
    }
    let doubles = |c: &Combiner, v: Operand, of: Operand| {
        binary_operator(c, v)
            .is_some_and(|(op, x, one, _)| op == BinaryOp::Shl && x == of && const_bits(c, one) == Some(1))
    };
    let signed_pred = if is_unsigned(pred) { flipped_signedness(pred) } else { pred };
    if doubles(c, op0, op1) {
        let zero = c.int(c.type_of(op1), 0);
        Some(new_icmp(c, inst, signed_pred, op1, zero))
    } else if doubles(c, op1, op0) {
        let zero = c.int(c.type_of(op0), 0);
        Some(new_icmp(c, inst, signed_pred, zero, op0))
    } else {
        None
    }
}

/// `X * Z pred Y * Z`: the shared multiplier is removed from the compare.
///
/// DEVIATE 3 (header).
fn fold_shared_multiplier(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
    side0: &Side,
    side1: &Side,
) -> Option<Visit> {
    let (_, m0, m1, _) = binary_operator(c, op0).filter(|&(op, ..)| op == BinaryOp::Mul)?;
    // m_Mul(m_Value(X), m_Value(Z)) or m_Mul(m_Value(Z), m_Value(X)), against
    // m_c_Mul(m_Specific(Z), m_Value(Y)).
    let (x, z, y) = [(m0, m1), (m1, m0)]
        .into_iter()
        .find_map(|(x, z)| commutative_with_specific(c, inst, op1, BinaryOp::Mul, z).map(|y| (x, z, y)))?;
    let q = query(c, Some(inst));
    let (nsw, nuw) = (side0.has_nsw && side1.has_nsw, side0.has_nuw && side1.has_nuw);
    let ty = c.type_of(z);
    if is_signed(pred) {
        if !nsw {
            return None;
        }
        let known = known::compute_known_bits(&q, z, 0);
        if known.is_strictly_positive() {
            return Some(new_icmp(c, inst, pred, x, y));
        }
        if known.is_negative() {
            return Some(new_icmp(c, inst, swapped(pred), x, y));
        }
        let simplified = |c: &Combiner, p| {
            simplify_icmp_operands(c.context, c.env.layout, c.function, p, x, y).and_then(|r| const_bits(c, r))
        };
        let zero = c.int(ty, 0);
        if simplified(c, IntPredicate::Slt) == Some(1) {
            return Some(new_icmp(c, inst, swapped(pred), z, zero));
        }
        if simplified(c, IntPredicate::Sgt) == Some(1) {
            return Some(new_icmp(c, inst, pred, z, zero));
        }
        return None;
    }
    let non_zero;
    if is_equality(pred) {
        // If X != Y, fold (X *nw Z) eq/ne (Y *nw Z) -> Z eq/ne 0
        if (nsw || nuw) && nonzero::is_known_non_equal(&q, x, y, 0) {
            let zero = c.int(ty, 0);
            return Some(new_icmp(c, inst, pred, z, zero));
        }
        let known = known::compute_known_bits(&q, z, 0);
        // if Z % 2 != 0: X * Z eq/ne Y * Z -> X eq/ne Y
        if known.count_max_trailing_zeros() == 0 {
            return Some(new_icmp(c, inst, pred, x, y));
        }
        non_zero = known.one != 0 || nonzero::is_known_non_zero(&q, z, 0);
        // if Z != 0 and nsw(X * Z) and nsw(Y * Z): X * Z eq/ne Y * Z -> X eq/ne
        // Y
        if non_zero && nsw {
            return Some(new_icmp(c, inst, pred, x, y));
        }
    } else {
        non_zero = nonzero::is_known_non_zero(&q, z, 0);
    }
    // If Z != 0 and nuw(X * Z) and nuw(Y * Z):
    //    X * Z u{lt/le/gt/ge}/eq/ne Y * Z -> X u{lt/le/gt/ge}/eq/ne Y
    if non_zero && nuw {
        return Some(new_icmp(c, inst, pred, x, y));
    }
    None
}

/// `icmp (srem X, Y), Y` and `icmp Y, (srem X, Y)`: the sign of `Y` decides.
fn fold_srem_against_divisor(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let srem_of = |c: &Combiner, rem: Operand, divisor: Operand| {
        binary_operator(c, rem).filter(|&(op, _, d, _)| op == BinaryOp::SRem && d == divisor)
    };
    // We don't check hasOneUse to avoid increasing register pressure because
    // the value we use is the same value this instruction was already using.
    let (divisor, pred) = if let Some((_, _, d, _)) = srem_of(c, op0, op1) {
        (d, swapped(pred))
    } else {
        let (_, _, d, _) = srem_of(c, op1, op0)?;
        (d, pred)
    };
    let ty = c.type_of(divisor);
    let i1 = c.context.types.int(1);
    match pred {
        IntPredicate::Eq => Some(Visit::Replace(c.int(i1, 0))),
        IntPredicate::Ne => Some(Visit::Replace(c.int(i1, 1))),
        IntPredicate::Sgt | IntPredicate::Sge => {
            let ones = c.all_ones(ty);
            Some(new_icmp(c, inst, IntPredicate::Sgt, divisor, ones))
        }
        IntPredicate::Slt | IntPredicate::Sle => {
            let zero = c.int(ty, 0);
            Some(new_icmp(c, inst, IntPredicate::Slt, divisor, zero))
        }
        _ => None,
    }
}

/// `ICmpInst::getSwappedPredicate`.
fn swapped(pred: IntPredicate) -> IntPredicate {
    match pred {
        IntPredicate::Eq | IntPredicate::Ne => pred,
        IntPredicate::Ugt => IntPredicate::Ult,
        IntPredicate::Uge => IntPredicate::Ule,
        IntPredicate::Ult => IntPredicate::Ugt,
        IntPredicate::Ule => IntPredicate::Uge,
        IntPredicate::Sgt => IntPredicate::Slt,
        IntPredicate::Sge => IntPredicate::Sle,
        IntPredicate::Slt => IntPredicate::Sgt,
        IntPredicate::Sle => IntPredicate::Sge,
    }
}

/// `m_c_<op>(m_Specific(other), m_Value(A))` of `v`: `A`.
fn commutative_with_specific(
    c: &Combiner,
    inst: InstId,
    v: Operand,
    op: BinaryOp,
    other: Operand,
) -> Option<Operand> {
    pattern::commutative_with(&query(c, Some(inst)), v, op, |x| x == other).map(|(_, _, a)| a)
}

/// `foldICmpXorXX`: a compare of `X ^ A` with `X`.
fn fold_icmp_xor_xx(
    c: &mut Combiner,
    inst: InstId,
    mut pred: IntPredicate,
    mut op0: Operand,
    mut op1: Operand,
) -> Option<Visit> {
    // Normalize the xor operand as operand 0.
    if commutative_with_specific(c, inst, op1, BinaryOp::Xor, op0).is_some() {
        std::mem::swap(&mut op0, &mut op1);
        pred = swapped(pred);
    }
    let a = commutative_with_specific(c, inst, op0, BinaryOp::Xor, op1)?;

    // icmp (X ^ Y_NonZero) u>= X --> icmp (X ^ Y_NonZero) u> X, and the like
    let strict = strict_predicate(pred);
    if strict != pred && nonzero::is_known_non_zero(&query(c, Some(inst)), a, 0) {
        return Some(new_icmp(c, inst, strict, op0, op1));
    }

    // These transform work when A is negative.
    // X s< X^A, X s<= X^A, X u> X^A, X u>= X^A  --> X s< 0
    // X s> X^A, X s>= X^A, X u< X^A, X u<= X^A  --> X s>= 0
    if let Some((bits, width)) = query(c, None).int_constant(a)
        && bits >> (width - 1) & 1 == 1
    {
        let new = match strict_predicate(pred) {
            IntPredicate::Slt | IntPredicate::Ugt => IntPredicate::Slt,
            IntPredicate::Sgt | IntPredicate::Ult => IntPredicate::Sge,
            _ => return None,
        };
        let zero = c.int(c.type_of(op0), 0);
        return Some(new_icmp(c, inst, new, op0, zero));
    }
    None
}

/// `foldICmpOrXX`: a compare of `X | A` with `X`.
fn fold_icmp_or_xx(
    c: &mut Combiner,
    inst: InstId,
    mut pred: IntPredicate,
    mut op0: Operand,
    mut op1: Operand,
) -> Option<Visit> {
    // Normalize or operand as operand 0.
    if commutative_with_specific(c, inst, op1, BinaryOp::Or, op0).is_some() {
        std::mem::swap(&mut op0, &mut op1);
        pred = swapped(pred);
    } else {
        commutative_with_specific(c, inst, op0, BinaryOp::Or, op1)?;
    }
    // icmp (X | Y) u<= X --> (X | Y) == X
    if pred == IntPredicate::Ule {
        return Some(new_icmp(c, inst, IntPredicate::Eq, op0, op1));
    }
    // icmp (X | Y) u> X --> (X | Y) != X
    if pred == IntPredicate::Ugt {
        return Some(new_icmp(c, inst, IntPredicate::Ne, op0, op1));
    }
    if is_equality(pred) && has_one_use(c, op0) {
        let a = commutative_with_specific(c, inst, op0, BinaryOp::Or, op1)?;
        let ty = c.type_of(op1);
        let mut consumes = false;
        // icmp (X | Y) eq/ne Y --> (X & ~Y) eq/ne 0 if Y is freely invertible
        let op1_inverts = !is_constant(c, op1) && !has_n_uses_or_more(c, op1, 3);
        if let Some(not_op1) = c.get_freely_inverted(inst, op1, op1_inverts, true, &mut consumes) {
            let and = c.create_binop(inst, BinaryOp::And, a, not_op1, Flags::default());
            let zero = c.int(ty, 0);
            return Some(new_icmp(c, inst, pred, and, zero));
        }
        // icmp (X | Y) eq/ne Y --> (~X | Y) eq/ne -1 if X  is freely
        // invertible.
        let a_inverts = has_one_use(c, a);
        if let Some(not_a) = c.get_freely_inverted(inst, a, a_inverts, true, &mut consumes) {
            let or = c.create_binop(inst, BinaryOp::Or, op1, not_a, Flags::default());
            let ones = c.all_ones(ty);
            return Some(new_icmp(c, inst, pred, or, ones));
        }
    }
    None
}

/// `m_OneUse(m_c_Add(m_Specific(base), m_Value(X)))` of `sum`: `X`.
fn one_use_commutative_add(
    c: &Combiner,
    sum: Operand,
    base: Operand,
) -> Option<Operand> {
    if !has_one_use(c, sum) {
        return None;
    }
    let (op, x, y, _) = binary_operator(c, sum)?;
    if op != BinaryOp::Add {
        return None;
    }
    if x == base {
        Some(y)
    } else if y == base {
        Some(x)
    } else {
        None
    }
}

/// The two `(Op + X) + C` forms of the unsigned-overflow compares.
fn fold_add_add_constant(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let imm = |c: &Combiner, v: Operand| pattern::imm_constant(&query(c, None), v);
    let inner = |c: &Combiner, outer: Operand, base: Operand| -> Option<(Operand, Operand)> {
        if !has_one_use(c, outer) {
            return None;
        }
        let (op, x, k, _) = binary_operator(c, outer)?;
        if op != BinaryOp::Add || !imm(c, k) {
            return None;
        }
        let x = one_use_commutative_add_free(c, x, base)?;
        Some((x, k))
    };
    if matches!(pred, IntPredicate::Ult | IntPredicate::Uge)
        && let Some((x, k)) = inner(c, op0, op1)
    {
        let not_k = c.int(c.type_of(k), !const_bits(c, k)?);
        let sub = c.create_binop(inst, BinaryOp::Sub, not_k, x, Flags::default());
        return Some(new_icmp(c, inst, pred, sub, op1));
    }
    if matches!(pred, IntPredicate::Ugt | IntPredicate::Ule)
        && let Some((x, k)) = inner(c, op1, op0)
    {
        let not_k = c.int(c.type_of(k), !const_bits(c, k)?);
        let sub = c.create_binop(inst, BinaryOp::Sub, not_k, x, Flags::default());
        return Some(new_icmp(c, inst, pred, op0, sub));
    }
    None
}

/// `m_c_Add(m_Specific(base), m_Value(X))` of `sum`, any number of uses.
fn one_use_commutative_add_free(
    c: &Combiner,
    sum: Operand,
    base: Operand,
) -> Option<Operand> {
    let (op, x, y, _) = binary_operator(c, sum)?;
    if op != BinaryOp::Add {
        return None;
    }
    if x == base {
        Some(y)
    } else if y == base {
        Some(x)
    } else {
        None
    }
}

/// `(X & -P2) == INT_MIN` is `X s< INT_MIN + P2`, and `!=` is `s>=`. The pin's
/// first InstCombine run takes it for any `and` with a negated power of two or
/// zero.
#[allow(clippy::too_many_arguments)]
fn fold_sign_mask_equality(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op: BinaryOp,
    x: Operand,
    mask_op: Operand,
    op1: Operand,
    ty: llrm_mir::types::TypeId,
) -> Option<Visit> {
    if op != BinaryOp::And || !is_equality(pred) {
        return None;
    }
    let (sign, width) = query(c, None).int_constant(op1)?;
    let (negated, _) = query(c, None).int_constant(mask_op)?;
    let positive = negated.wrapping_neg() & mask(width);
    if sign != 1u128 << (width - 1) || !(positive == 0 || positive.is_power_of_two()) {
        return None;
    }
    let bound = c.int(ty, sign.wrapping_add(positive) & mask(width));
    let new = if pred == IntPredicate::Eq { IntPredicate::Slt } else { IntPredicate::Sge };
    Some(new_icmp(c, inst, new, x, bound))
}

/// `((Op1 + M) & M) u< Op1` is `Op1 != 0` for a low-bit mask `M`, `u>=` is
/// `== 0`; `Op0 u> ((Op0 + M) & M)` is `Op0 != 0`, `u<=` is `== 0`.
fn fold_low_bit_mask_overflow(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
    ty: llrm_mir::types::TypeId,
) -> Option<Visit> {
    let (masked, other, new) = match pred {
        IntPredicate::Ult => (op0, op1, IntPredicate::Ne),
        IntPredicate::Uge => (op0, op1, IntPredicate::Eq),
        IntPredicate::Ugt => (op1, op0, IntPredicate::Ne),
        IntPredicate::Ule => (op1, op0, IntPredicate::Eq),
        _ => return None,
    };
    let (BinaryOp::And, summed, low, _) = binary_operator(c, masked)? else { return None };
    let (bits, width) = query(c, None).int_constant(low)?;
    if bits == 0 || bits & bits.wrapping_add(1) & mask(width) != 0 {
        return None;
    }
    let (BinaryOp::Add, base, added, _) = binary_operator(c, summed)? else { return None };
    if base != other || query(c, None).int_constant(added)?.0 != bits {
        return None;
    }
    let zero = c.int(ty, 0);
    Some(new_icmp(c, inst, new, other, zero))
}

fn const_bits(
    c: &Combiner,
    k: Operand,
) -> Option<u128> {
    query(c, None).int_constant(k).map(|(bits, _)| bits)
}

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

/// `isa<Constant>`: an integer constant here.
fn is_constant(
    c: &Combiner,
    v: Operand,
) -> bool {
    matches!(v, Operand::Constant(_)) || query(c, None).int_constant(v).is_some()
}

/// `Value::hasNUsesOrMore`.
fn has_n_uses_or_more(
    c: &Combiner,
    v: Operand,
    n: usize,
) -> bool {
    match v {
        Operand::Value(value) => c.function.users(value).iter().take(n).count() >= n,
        _ => false,
    }
}
