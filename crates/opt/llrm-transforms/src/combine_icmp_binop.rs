//! InstCombine's `foldICmpBinOp`, the add and subtract arms
//! ([`InstCombineCompares.cpp:5301`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L5301)
//! at d1106deb71cc).
//!
//! MIRROR: the order of the steps and each condition, up to the `Xor`/`Or`
//! folds.
//!
//! OWED, in place as `// OWED:` comments: `foldICmpXNegX`, the sign-mask and
//! low-bit-mask forms, `foldICmpXorXX`, `foldICmpOrXX`, the shared multiplier,
//! `srem`, and every arm after them (`udiv`, `shl`, `lshr`, `and`...).
//!
//! DEVIATE 1: `ShareCommonDivisor` takes only its fast path (the offset is 1,
//! or -1 for the negative forms); the general case needs `isMultipleOf`, which
//! MIR's value tracking does not have. Exit: `isMultipleOf`.
//!
//! DEVIATE 2: no vectors; the constants of the equality fold are integers.
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
use llrm_mir::valuetracking::{nonzero, pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{has_one_use, query};

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
    // OWED: foldICmpXNegX.

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
    // OWED: the sign-mask equality and the low-bit-mask overflow forms.

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
    // OWED: icmp (0-X) < cst, foldICmpXorXX, foldICmpOrXX, the shared
    // multiplier, srem and the arms after it.
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
