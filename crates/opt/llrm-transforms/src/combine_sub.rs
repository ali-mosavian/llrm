//! `InstCombinerImpl::visitSub`: the integer subtraction folds.
//!
//! Translation of
//! [`visitSub`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp#L2413)
//! in the order it reaches them. `-x` is sunk into its operand by
//! [`combine_negator`](crate::combine_negator).
//!
//! MIRROR: the C++ order, including branches a later fold shadows.
//!
//! DEVIATE 1: no vectors: `foldVectorBinop` and the `vector_reduce_add` fold
//! (:2602) are not translated.
//!
//! DEVIATE 2: `isFreeToInvert` (`~X - ~Y`, :2587),
//! `tryFoldInstWithCtpopWithNot`, `foldSubOfMinMax` and everything from the
//! select-of-sub fold (:2799) on are the next strip, but the `usub.sat`,
//! `umin` and `umax` identities (:2978-3006).
//!
//! DEVIATE 3: constant expressions are skipped (`combine_addsub` DEVIATE 4).
//!
//! # How it works
//! After the shared prologue (simplify, phi operands, `x - -y`, shl
//! factoring), a constant minuend is folded into an add/select/phi, a
//! negation is sunk, and the rest are pattern rewrites over `add`, `sub`,
//! `and`, `or`, `xor` of shared operands. Each rewrite builds with the
//! `Combiner` builder.
//!
//! # Used by
//! `combine::visit` (the `Sub` arm).
//!
//! # Example
//! `(x | y) - (x & y)` is `x ^ y`:
//!
//! ```text
//! %o = or i32 %x, %y
//! %a = and i32 %x, %y
//! %r = sub i32 %o, %a   ; %r = xor i32 %x, %y
//! ```

use llrm_mir::context::mask;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::valuetracking::{known, pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_addsub::{add_to, factorize_math_with_shl_ops, will_not_overflow_signed_sub};
use crate::combine_distribute::fold_using_distributive_laws;
use crate::combine_muldivrem::number::is_negated_power2;
use crate::combine_shared::{binop, commutative, fold_constants, has_one_use, parts, query, simplify_visited, width};

/// The `nsw` of `flags` when `keep`.
fn nsw_if(keep: bool) -> Flags {
    if keep { Flags::NSW } else { Flags::default() }
}

/// `m_Constant` of an integer: its bits and width.
fn int_constant(
    c: &Combiner,
    v: Operand,
) -> Option<(u128, u32)> {
    pattern::apint(&query(c, None), v)
}

/// `-v` of a constant (`ConstantExpr::getNeg`).
fn negated_constant(
    c: &Combiner,
    v: Operand,
) -> Option<Operand> {
    let zero = c.int(c.type_of(v), 0);
    fold_constants(c, BinaryOp::Sub, zero, v)
}

/// `x - -y` is `x + y`, keeping `nsw` (:2430): `dyn_castNegVal(Op1)`.
fn fold_sub_of_negation(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
    flags: Flags,
) -> Option<Visit> {
    let neg = pattern::neg(&query(c, None), op1);
    let (value, nsw) = if let Some((_, y, inner)) = neg {
        (y, inner.contains(Flags::NSW) && flags.contains(Flags::NSW))
    } else {
        let (bits, w) = int_constant(c, op1)?;
        (negated_constant(c, op1)?, bits != 1u128 << (w - 1) && flags.contains(Flags::NSW))
    };
    Some(c.replace_with_binop(inst, BinaryOp::Add, op0, value, nsw_if(nsw)))
}

/// `C - (X + C2)` is `(C - C2) - X` (:2455).
fn fold_constant_minus_add(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
    flags: Flags,
) -> Option<Visit> {
    int_constant(c, op0)?;
    let (x, c2, add_flags) = {
        let q = query(c, Some(inst));
        let (x, c2) = pattern::add_like(&q, op1, Flags::default())?;
        pattern::apint(&q, c2)?;
        // `or disjoint` is `add nuw nsw`.
        let add_flags = pattern::binary(&q, op1, BinaryOp::Add).map(|(_, one, ..)| one.flags);
        (x, c2, add_flags)
    };
    let difference = fold_constants(c, BinaryOp::Sub, op0, c2)?;
    let will_not_overflow = will_not_overflow_signed_sub(c, inst, op0, c2);
    let (op1_nsw, op1_nuw) = add_flags.map_or((true, true), |f| (f.contains(Flags::NSW), f.contains(Flags::NUW)));
    let mut result = Flags::default();
    if flags.contains(Flags::NSW) && op1_nsw && will_not_overflow {
        result.insert(Flags::NSW);
    }
    if flags.contains(Flags::NUW) && op1_nuw {
        result.insert(Flags::NUW);
    }
    Some(c.replace_with_binop(inst, BinaryOp::Sub, difference, x, result))
}

/// `none_of(I.users(), m_c_Select(m_Specific(Op1), m_Specific(&I)))`: the
/// negation is not read by an abs/nabs-shaped select.
fn is_abs_select_operand(
    c: &Combiner,
    inst: InstId,
    op1: Operand,
) -> bool {
    let result = c.function.instruction(inst).result.expect("a value");
    let me = Operand::Value(result);
    c.function.users(result).iter().any(|one| {
        let user = c.function.instruction(one.user);
        user.opcode == Opcode::Select
            && ((user.operands[1], user.operands[2]) == (op1, me) || (user.operands[1], user.operands[2]) == (me, op1))
    })
}

/// `visitSub`'s negation: `sub a, b` as `add a, (sub 0, b)` with the negation
/// sunk into `b`, then the flags `TryToNarrowDeduceFlags` infers (:2487).
fn fold_with_negator(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
    flags: Flags,
) -> Option<Visit> {
    let is_negation = pattern::zero_int(&query(c, None), op0);
    if !is_negation || !is_abs_select_operand(c, inst, op1) {
        let nsw = is_negation && flags.contains(Flags::NSW);
        if let Some(negated) = c.negate(is_negation, nsw, op1) {
            return Some(c.replace_with_binop(inst, BinaryOp::Add, negated, op0, Flags::default()));
        }
    }
    if !is_negation {
        return None;
    }
    // Should have been handled in Negator: narrow, else infer the flags.
    Some(infer_sub_flags(c, inst, op0, op1, flags))
}

/// `TryToNarrowDeduceFlags`: the narrowing, else `nsw`/`nuw` when
/// `willNotOverflow{Signed,Unsigned}Sub`.
fn infer_sub_flags(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
    flags: Flags,
) -> Visit {
    use llrm_mir::valuetracking::range::{OverflowResult, compute_overflow_for_unsigned_sub};
    if let Some(narrowed) = c.narrow_math_if_no_overflow(inst) {
        return narrowed;
    }
    let mut new = flags;
    if !flags.contains(Flags::NSW) && will_not_overflow_signed_sub(c, inst, op0, op1) {
        new.insert(Flags::NSW);
    }
    let unsigned = compute_overflow_for_unsigned_sub(&query(c, Some(inst)), op0, op1) == OverflowResult::NeverOverflows;
    if !flags.contains(Flags::NUW) && unsigned {
        new.insert(Flags::NUW);
    }
    if new == flags {
        return Visit::None;
    }
    c.function.set_flags(inst, new);
    Visit::Changed
}

/// `-1 - A` is `~A`; `(X + -1) - Y` is `~Y + X` (:2508).
fn fold_not_forms(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    if pattern::all_ones(&query(c, None), op0) {
        let ones = c.all_ones(c.type_of(op1));
        return Some(c.replace_with_binop(inst, BinaryOp::Xor, op1, ones, Flags::default()));
    }
    let x = {
        let q = query(c, Some(inst));
        let (x, minus_one, _) = binop(&q, op0, BinaryOp::Add)?;
        (has_one_use(c, op0) && pattern::all_ones(&q, minus_one)).then_some(x)?
    };
    let not = c.create_not(inst, op1);
    Some(c.replace_with_binop(inst, BinaryOp::Add, not, x, Flags::default()))
}

/// `(X & C1) - (X & C2)` with `(C1 & C2) == C2` is `X & (C1 ^ C2)` (:2519).
fn fold_masked_difference(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (x, c1, c2) = {
        let q = query(c, Some(inst));
        let (x, c1, _) = binop(&q, op0, BinaryOp::And)?;
        let (x1, c2, _) = binop(&q, op1, BinaryOp::And)?;
        let (k1, _) = pattern::apint(&q, c1)?;
        let (k2, _) = pattern::apint(&q, c2)?;
        (x1 == x && k1 & k2 == k2).then_some((x, k1, k2))?
    };
    let k = c.int(c.type_of(x), c1 ^ c2);
    Some(c.replace_with_binop(inst, BinaryOp::And, x, k, Flags::default()))
}

/// The reassociations of a sub of a sub or an add (:2531).
fn fold_reassociated_sub(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
    flags: Flags,
) -> Option<Visit> {
    // ((X - Y) + Z) - Op1 --> (X + Z) - (Y + Op1)
    let sub_plus = {
        let q = query(c, Some(inst));
        if has_one_use(c, op0) {
            commutative(&q, op0, BinaryOp::Add, |sub| has_one_use(c, sub) && binop(&q, sub, BinaryOp::Sub).is_some())
                .and_then(|(sub, z)| binop(&q, sub, BinaryOp::Sub).map(|(x, y, _)| (x, y, z)))
        } else {
            None
        }
    };
    if let Some((x, y, z)) = sub_plus {
        let xz = c.create_add(inst, x, z);
        let yw = c.create_add(inst, y, op1);
        return Some(c.replace_with_binop(inst, BinaryOp::Sub, xz, yw, Flags::default()));
    }
    // ((X - Y) - Op1) --> X - (Y + Op1)
    let inner = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::Sub).filter(|_| has_one_use(c, op0))
    };
    if let Some((x, y, left)) = inner {
        let nuw = flags.contains(Flags::NUW) && left.contains(Flags::NUW);
        let nsw = nuw && flags.contains(Flags::NSW) && left.contains(Flags::NSW);
        let mut wrap = Flags::default();
        if nuw {
            wrap.insert(Flags::NUW);
        }
        if nsw {
            wrap.insert(Flags::NSW);
        }
        let add = c.create_binop(inst, BinaryOp::Add, y, op1, wrap);
        return Some(c.replace_with_binop(inst, BinaryOp::Sub, x, add, wrap));
    }
    // (X + C0) - (Y + C1) --> (X - Y) + (C0 - C1)
    let constants = {
        let q = query(c, Some(inst));
        let one_use_add = |v: Operand| {
            let (x, k, _) = binop(&q, v, BinaryOp::Add)?;
            (has_one_use(c, v) && pattern::apint(&q, k).is_some()).then_some((x, k))
        };
        one_use_add(op0).zip(one_use_add(op1))
    };
    if let Some(((x, cx), (y, cy))) = constants
        && let Some(difference) = fold_constants(c, BinaryOp::Sub, cx, cy)
    {
        let ops_sub = c.create_sub(inst, x, y);
        return Some(c.replace_with_binop(inst, BinaryOp::Add, ops_sub, difference, Flags::default()));
    }
    // (X + Z) - (Y + Z) --> (X - Y)
    let shared = {
        let q = query(c, Some(inst));
        pattern::add_like(&q, op0, Flags::default())
            .zip(pattern::add_like(&q, op1, Flags::default()))
            .map(
                |((w, x), (y, z))| {
                    let wrap = |v: Operand, flag: Flags| pattern::add_like(&q, v, flag).is_some();
                    let kept = if w == y {
                        Some((x, z))
                    } else if w == z {
                        Some((x, y))
                    } else if x == y {
                        Some((w, z))
                    } else if x == z {
                        Some((w, y))
                    } else {
                        None
                    };
                    (kept, wrap(op0, Flags::NSW) && wrap(op1, Flags::NSW), wrap(op1, Flags::NUW))
                },
            )
    };
    if let Some((Some((a, b)), both_nsw, op1_nuw)) = shared {
        let mut wrap = Flags::default();
        if flags.contains(Flags::NSW) && both_nsw {
            wrap.insert(Flags::NSW);
        }
        if flags.contains(Flags::NUW) && op1_nuw {
            wrap.insert(Flags::NUW);
        }
        return Some(c.replace_with_binop(inst, BinaryOp::Sub, a, b, wrap));
    }
    None
}

/// A constant minuend: the extension, `not`, select, phi and sub forms
/// (:2610).
fn fold_constant_minuend(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    if !matches!(op0, Operand::Constant(_)) {
        return None;
    }
    let ty = c.type_of(op0);
    let (boolean_zext, boolean_sext, not_x, sub_from) = {
        let q = query(c, Some(inst));
        let boolean = |op: CastOp| pattern::cast(&q, op1, op).map(|(_, _, x)| x).filter(|&x| width(c, x) == 1);
        let sub_from = binop(&q, op1, BinaryOp::Sub)
            .filter(|&(c2, ..)| pattern::apint(&q, c2).is_some())
            .map(|(c2, x, _)| (c2, x));
        (boolean(CastOp::ZExt), boolean(CastOp::SExt), pattern::not(&q, op1), sub_from)
    };
    // C - (zext bool) --> bool ? C - 1 : C
    if let Some(x) = boolean_zext {
        let minus_one = add_to(c, op0, -1)?;
        return Some(c.replace_with_new(inst, Opcode::Select, ty, vec![x, minus_one, op0], Flags::default()));
    }
    // C - (sext bool) --> bool ? C + 1 : C
    if let Some(x) = boolean_sext {
        let plus_one = add_to(c, op0, 1)?;
        return Some(c.replace_with_new(inst, Opcode::Select, ty, vec![x, plus_one, op0], Flags::default()));
    }
    // C - ~X == X + (1+C)
    if let Some(x) = not_x {
        let plus_one = add_to(c, op0, 1)?;
        return Some(c.replace_with_binop(inst, BinaryOp::Add, x, plus_one, Flags::default()));
    }
    // Try to fold the constant sub into select arguments and phi values.
    if let Some(select) = defining_with(c, op1, |o| *o == Opcode::Select) {
        let found = c.fold_op_into_select(inst, select, false, false);
        if found != Visit::None {
            return Some(found);
        }
    }
    if let Some(phi) = defining_with(c, op1, |o| *o == Opcode::Phi) {
        let found = c.fold_op_into_phi(inst, phi, false);
        if found != Visit::None {
            return Some(found);
        }
    }
    // C-(C2-X) --> X+(C-C2)
    let (c2, x) = sub_from?;
    let difference = fold_constants(c, BinaryOp::Sub, op0, c2)?;
    Some(c.replace_with_binop(inst, BinaryOp::Add, x, difference, Flags::default()))
}

/// The instruction defining `v` if its opcode satisfies `is`.
fn defining_with(
    c: &Combiner,
    v: Operand,
    is: impl Fn(&Opcode) -> bool,
) -> Option<InstId> {
    let def = crate::combine_shared::inst_def(c, v)?;
    is(&c.function.instruction(def).opcode).then_some(def)
}

/// An integer-constant minuend (:2689): a mask minuend is an `xor` when the
/// subtrahend has no other bits, and the masked-sub identity.
fn fold_constant_mask_minuend(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (k, w) = int_constant(c, op0)?;
    if k != 0 && k & k.wrapping_add(1) & mask(w) == 0 {
        // Turn this into a xor if LHS is 2^n-1 and the remaining bits are
        // known zero. Dominating conditions are not used (SQ without DC).
        let mut q = query(c, Some(inst));
        q.dom_conditions = None;
        let zero = known::compute_known_bits(&q, op1, 0).zero;
        if (k | zero) & mask(w) == mask(w) {
            return Some(c.replace_with_binop(inst, BinaryOp::Xor, op1, op0, Flags::default()));
        }
    }
    // C - ((C3 -nuw X) & C2) --> (C - (C2 & C3)) + (X & C2)
    let (x, c2, c3) = {
        let q = query(c, Some(inst));
        if !has_one_use(c, op1) {
            return None;
        }
        let (inner, c2, _) = binop(&q, op1, BinaryOp::And)?;
        let (k2, _) = pattern::apint(&q, c2)?;
        let (c3, x, sub_flags) = binop(&q, inner, BinaryOp::Sub)?;
        let (k3, _) = pattern::apint(&q, c3)?;
        let negated_power2 = is_negated_power2(k2, w);
        (sub_flags.contains(Flags::NUW) || negated_power2).then_some((x, k2, k3))?
    };
    let and_c = c2 & c3 & mask(w);
    let and_minus_one = and_c.wrapping_sub(1) & mask(w);
    let add = c2.wrapping_add(c3) & mask(w);
    let diff = c3.wrapping_sub(and_minus_one) & mask(w);
    if diff.is_power_of_two() && and_minus_one & !add == 0 {
        let ty = c.type_of(op0);
        let mask_c = c.int(ty, c2);
        let and = c.create_binop(inst, BinaryOp::And, x, mask_c, Flags::default());
        let base = c.int(ty, k.wrapping_sub(and_c) & mask(w));
        return Some(c.replace_with_binop(inst, BinaryOp::Add, and, base, Flags::default()));
    }
    None
}

/// `X - (X + Y)` and `(X - Y) - X` are `-Y` (:2742).
fn fold_negated_remainder(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let y = {
        let q = query(c, Some(inst));
        commutative(&q, op1, BinaryOp::Add, |x| x == op0)
            .map(|(_, y)| y)
            .or_else(|| binop(&q, op0, BinaryOp::Sub).filter(|&(x, ..)| x == op1).map(|(_, y, _)| y))?
    };
    let zero = c.int(c.type_of(y), 0);
    Some(c.replace_with_binop(inst, BinaryOp::Sub, zero, y, Flags::default()))
}

/// The `or`/`and`/`xor` identities of a difference (:2752).
fn fold_bitwise_difference(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    use BinaryOp::{Add, And, Or, Xor};
    let pair = |q: &llrm_mir::valuetracking::Query, v: Operand, op: BinaryOp| binop(q, v, op).map(|(a, b, _)| (a, b));
    let same = |q: &llrm_mir::valuetracking::Query, v: Operand, op: BinaryOp, a: Operand, b: Operand| {
        pair(q, v, op).is_some_and(|(x, y)| (x, y) == (a, b) || (x, y) == (b, a))
    };
    let found = {
        let q = query(c, Some(inst));
        let either_use = has_one_use(c, op0) || has_one_use(c, op1);
        // (or A, B) - (and A, B) --> (xor A, B)
        if let Some((a, b)) = pair(&q, op1, And).filter(|&(a, b)| same(&q, op0, Or, a, b)) {
            Some((Xor, a, b, false))
        // (add A, B) - (or A, B) --> (and A, B)
        } else if let Some((a, b)) = pair(&q, op0, Add).filter(|&(a, b)| same(&q, op1, Or, a, b)) {
            Some((And, a, b, false))
        // (add A, B) - (and A, B) --> (or A, B)
        } else if let Some((a, b)) = pair(&q, op0, Add).filter(|&(a, b)| same(&q, op1, And, a, b)) {
            Some((Or, a, b, false))
        // (and A, B) - (or A, B) --> neg (xor A, B)
        } else if let Some((a, b)) = pair(&q, op0, And).filter(|&(a, b)| same(&q, op1, Or, a, b) && either_use) {
            Some((Xor, a, b, true))
        // (or A, B) - (xor A, B) --> (and A, B)
        } else if let Some((a, b)) = pair(&q, op1, Xor).filter(|&(a, b)| same(&q, op0, Or, a, b)) {
            Some((And, a, b, false))
        // (xor A, B) - (or A, B) --> neg (and A, B)
        } else {
            pair(&q, op0, Xor).filter(|&(a, b)| same(&q, op1, Or, a, b) && either_use).map(|(a, b)| (And, a, b, true))
        }
    };
    let (op, a, b, negated) = found?;
    if negated {
        let logic = c.create_binop(inst, op, a, b, Flags::default());
        let zero = c.int(c.type_of(logic), 0);
        return Some(c.replace_with_binop(inst, BinaryOp::Sub, zero, logic, Flags::default()));
    }
    Some(c.replace_with_binop(inst, op, a, b, Flags::default()))
}

/// The `and` of the subtrahend forms (:2812): `(X | Y) - X`,
/// `(and X, -Y) - X` and `(and X, C) - X`.
fn fold_and_difference(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    // ((X | Y) - X) --> (~X & Y)
    let y = {
        let q = query(c, Some(inst));
        if has_one_use(c, op0) { commutative(&q, op0, BinaryOp::Or, |x| x == op1).map(|(_, y)| y) } else { None }
    };
    if let Some(y) = y {
        let not = c.create_not(inst, op1);
        return Some(c.replace_with_binop(inst, BinaryOp::And, y, not, Flags::default()));
    }
    // (sub (and Op1, (neg X)), Op1) --> neg (and Op1, (add X, -1))
    let x = {
        let q = query(c, Some(inst));
        if has_one_use(c, op0) {
            commutative(&q, op0, BinaryOp::And, |a| a == op1)
                .map(|(_, other)| other)
                .filter(|&n| has_one_use(c, n))
                .and_then(|n| pattern::neg(&q, n).map(|(_, x, _)| x))
        } else {
            None
        }
    };
    if let Some(x) = x {
        let ones = c.all_ones(c.type_of(x));
        let dec = c.create_add(inst, x, ones);
        let and = c.create_binop(inst, BinaryOp::And, op1, dec, Flags::default());
        let zero = c.int(c.type_of(and), 0);
        return Some(c.replace_with_binop(inst, BinaryOp::Sub, zero, and, Flags::default()));
    }
    // (sub (and Op1, C), Op1) --> neg (and Op1, ~C)
    let k = {
        let q = query(c, Some(inst));
        if has_one_use(c, op0) {
            binop(&q, op0, BinaryOp::And).filter(|&(a, ..)| a == op1).and_then(|(_, k, _)| pattern::apint(&q, k))
        } else {
            None
        }
    };
    if let Some((bits, w)) = k {
        let inverted = c.int(c.type_of(op1), !bits & mask(w));
        let and = c.create_binop(inst, BinaryOp::And, op1, inverted, Flags::default());
        let zero = c.int(c.type_of(and), 0);
        return Some(c.replace_with_binop(inst, BinaryOp::Sub, zero, and, Flags::default()));
    }
    None
}

/// `(sub (xor X, (sext C)), (sext C))` is `select C, (neg X), X` (:2845).
fn fold_xor_of_sext_bool(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let found = {
        let q = query(c, Some(inst));
        let shape = |lhs: Operand, rhs: Operand| {
            let (_, x) = if has_one_use(c, lhs) { commutative(&q, lhs, BinaryOp::Xor, |s| s == rhs) } else { None }?;
            let (_, _, cond) = pattern::cast(&q, rhs, CastOp::SExt)?;
            (width(c, cond) == 1).then_some((cond, x))
        };
        shape(op0, op1).map(|(cond, x)| (cond, x, true)).or_else(|| shape(op1, op0).map(|(cond, x)| (cond, x, false)))
    };
    let (cond, x, negated_first) = found?;
    let ty = c.type_of(x);
    let neg_x = {
        let zero = c.int(ty, 0);
        c.create_binop(inst, BinaryOp::Sub, zero, x, Flags::default())
    };
    let (t, f) = if negated_first { (neg_x, x) } else { (x, neg_x) };
    Some(c.replace_with_new(inst, Opcode::Select, ty, vec![cond, t, f], Flags::default()))
}

/// `InstCombinerImpl::visitSub` (:2413).
/// The `usub.sat`/`umin`/`umax` identities of a difference (:2978-3006).
fn fold_saturating_difference(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let ty = c.type_of(op0);
    // The operands the one-use call `v` of `base` takes where one of them is
    // `with`: the other one.
    let other_of = |c: &Combiner, v: Operand, base: &str, with: Operand| {
        let q = query(c, Some(inst));
        let (_, args) = pattern::intrinsic_call(&q, v, base).filter(|_| has_one_use(c, v))?;
        let [a, b] = *args else { return None };
        (a == with).then_some(b).or_else(|| (b == with).then_some(a))
    };
    // X - usub.sat(X, Y) => umin(X, Y)
    let usub = {
        let q = query(c, Some(inst));
        pattern::intrinsic_call(&q, op1, "llvm.usub.sat")
            .filter(|_| has_one_use(c, op1))
            .and_then(
                |(_, args)| match *args {
                    [a, y] if a == op0 => Some(y),
                    _ => None,
                },
            )
    };
    if let Some(y) = usub {
        return Some(Visit::Replace(c.create_intrinsic(inst, "umin", ty, &[op0, y])));
    }
    // umax(X, Op1) - Op1 --> usub.sat(X, Op1)
    if let Some(x) = other_of(c, op0, "llvm.umax", op1) {
        return Some(Visit::Replace(c.create_intrinsic(inst, "usub.sat", ty, &[x, op1])));
    }
    // Op0 - umin(X, Op0) --> usub.sat(Op0, X)
    if let Some(x) = other_of(c, op1, "llvm.umin", op0) {
        return Some(Visit::Replace(c.create_intrinsic(inst, "usub.sat", ty, &[op0, x])));
    }
    // Op0 - umax(X, Op0) --> 0 - usub.sat(X, Op0)
    if let Some(x) = other_of(c, op1, "llvm.umax", op0) {
        let sat = c.create_intrinsic(inst, "usub.sat", ty, &[x, op0]);
        let zero = c.int(ty, 0);
        return Some(c.replace_with_binop(inst, BinaryOp::Sub, zero, sat, Flags::default()));
    }
    // umin(X, Op1) - Op1 --> 0 - usub.sat(Op1, X)
    if let Some(x) = other_of(c, op0, "llvm.umin", op1) {
        let sat = c.create_intrinsic(inst, "usub.sat", ty, &[op1, x]);
        let zero = c.int(ty, 0);
        return Some(c.replace_with_binop(inst, BinaryOp::Sub, zero, sat, Flags::default()));
    }
    None
}

pub fn visit_sub(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    // simplifySubInst
    if let Some(found) = simplify_visited(c, inst) {
        return found;
    }
    // DEVIATE 1: foldVectorBinop.
    let found = c.fold_binop_with_phi_operands(inst);
    if !matches!(found, Visit::None) {
        return found;
    }
    let (op0, op1, flags) = parts(c, inst);
    // If this is a 'B = x-(-A)', change to B = x+A.
    if let Some(found) = fold_sub_of_negation(c, inst, op0, op1, flags) {
        return found;
    }
    // Try this before Negator to preserve NSW flag.
    if let Some(found) = factorize_math_with_shl_ops(c, inst) {
        return found;
    }
    if let Some(found) = fold_constant_minus_add(c, inst, op0, op1, flags) {
        return found;
    }
    if let Some(found) = fold_with_negator(c, inst, op0, op1, flags) {
        return found;
    }
    if let Some(v) = fold_using_distributive_laws(c, inst) {
        return Visit::Replace(v);
    }
    if width(c, op0) == 1 {
        return c.replace_with_binop(inst, BinaryOp::Xor, op0, op1, Flags::default());
    }
    if let Some(found) = fold_not_forms(c, inst, op0, op1) {
        return found;
    }
    if let Some(found) = fold_masked_difference(c, inst, op0, op1) {
        return found;
    }
    if let Some(found) = fold_reassociated_sub(c, inst, op0, op1, flags) {
        return found;
    }
    // DEVIATE 2: isFreeToInvert (~X - ~Y); DEVIATE 1: vector_reduce_add.
    let steps: [fn(&mut Combiner, InstId, Operand, Operand) -> Option<Visit>; 7] = [
        fold_constant_minuend,
        fold_constant_mask_minuend,
        fold_negated_remainder,
        fold_bitwise_difference,
        fold_and_difference,
        fold_xor_of_sext_bool,
        fold_saturating_difference,
    ];
    if let Some(found) = steps.iter().find_map(|step| step(c, inst, op0, op1)) {
        return found;
    }
    infer_sub_flags(c, inst, op0, op1, flags)
}

#[cfg(test)]
#[path = "combine_sub_tests.rs"]
mod tests;
