//! `visitAdd` after `x + x`: the negation and not identities, the
//! reassociations of an add with an add or sub, and the bitwise identities.
//!
//! Translation of the middle of
//! [`InstCombinerImpl::visitAdd`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp#L1645)
//! (`-A + -B` through `(or A, B) + (and A, B)`, :1646-:1819) and of
//! [`checkForNegativeOperand`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp#L752).
//!
//! MIRROR: the C++ order. Each fold is one function; `fold_add_rest` tries
//! them in the order `visitAdd` reaches them.
//!
//! DEVIATE 1: `SimplifyAddWithRemainder` (:1739), the align-up and
//! `(A & -A) - 1` idioms, `~(A*C)+A`, signum, `select` of `icmp eq`,
//! ceiling division, `~X + ~Y` (`isFreeToInvert`) and everything after
//! `canonicalizeLowbitMask` are the next strip.
//!
//! DEVIATE 2: `sext(A<B) + zext(A>B)` (:1766) and `X + (or X, -X)`'s
//! sibling folds that build `umax`/`scmp` need `CreateIntrinsic`
//! (`combine_build` DEVIATE 2).
//!
//! # How it works
//! Each fold matches with `pattern` and the shared helpers, builds the new
//! operands with the `Combiner` builder (which simplifies first) and returns
//! the `Visit` of `BinaryOperator::Create`.
//!
//! # Used by
//! `combine_addsub::visit_add`.
//!
//! # Example
//! `-a + b` is `b - a`:
//!
//! ```text
//! %n = sub i32 0, %a
//! %r = add i32 %n, %b   ; %r = sub i32 %b, %a
//! ```

use llrm_mir::context::mask;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::valuetracking::known::{NoCommonBitsSetResult, get_no_common_bits_set_result};
use llrm_mir::valuetracking::{Query, pattern, signbits};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{binop, commutative, fold_constants, has_one_use, parts, query, replace_operand, width};

/// `m_Add(m_Value(X), m_One())`: `X`.
fn plus_one(
    q: &Query,
    v: Operand,
) -> Option<Operand> {
    binop(q, v, BinaryOp::Add).filter(|&(_, one, _)| pattern::one(q, one)).map(|(x, ..)| x)
}

/// `m_c_And(m_Specific(a), m_Specific(b))` (either order).
fn is_and_of(
    q: &Query,
    v: Operand,
    op: BinaryOp,
    a: Operand,
    b: Operand,
) -> bool {
    binop(q, v, op).is_some_and(|(x, y, _)| (x, y) == (a, b) || (x, y) == (b, a))
}

/// The wrap flags of `flags` when `keep` is set.
fn nsw_if(keep: bool) -> Flags {
    if keep { Flags::NSW } else { Flags::default() }
}

/// `-A + -B` is `-(A + B)`; `-A + B` is `B - A`; `A + -B` is `A - B` (:1678).
fn fold_add_of_negation(
    c: &mut Combiner,
    inst: InstId,
    lhs: Operand,
    rhs: Operand,
    flags: Flags,
) -> Option<Visit> {
    let (neg_lhs, neg_rhs) = {
        let q = query(c, Some(inst));
        (pattern::neg(&q, lhs), pattern::neg(&q, rhs))
    };
    if let Some((_, a, left)) = neg_lhs {
        if let Some((_, b, _)) = neg_rhs {
            let sum = c.create_add(inst, a, b);
            let zero = c.int(c.type_of(sum), 0);
            return Some(c.replace_with_binop(inst, BinaryOp::Sub, zero, sum, Flags::default()));
        }
        let keep = flags.contains(Flags::NSW) && left.contains(Flags::NSW);
        return Some(c.replace_with_binop(inst, BinaryOp::Sub, rhs, a, nsw_if(keep)));
    }
    let (_, b, right) = neg_rhs?;
    let keep = flags.contains(Flags::NSW) && right.contains(Flags::NSW);
    Some(c.replace_with_binop(inst, BinaryOp::Sub, lhs, b, nsw_if(keep)))
}

/// `checkForNegativeOperand` (:752): an operand that is `-(Z & C)` or
/// `-(Z | ~C)` written with xor and `+ 1` turns the add into a sub.
fn check_for_negative_operand(
    c: &mut Combiner,
    inst: InstId,
    lhs: Operand,
    rhs: Operand,
) -> Option<Operand> {
    // The two new instructions replace the add: one of the operands must die.
    if !has_one_use(c, lhs) && !has_one_use(c, rhs) {
        return None;
    }
    let xor_const = |q: &Query, v: Operand| {
        binop(q, v, BinaryOp::Xor).and_then(|(y, k, _)| pattern::apint(q, k).map(|(k, w)| (y, k, w)))
    };
    let found = {
        let q = query(c, Some(inst));
        // if ONE is on the other side, swap
        let (l, r) = if plus_one(&q, rhs).is_some() { (rhs, lhs) } else { (lhs, rhs) };
        let from_plus_one = plus_one(&q, l).and_then(|mut x| {
            let mut other = r;
            // if XOR on the other side, swap
            if xor_const(&q, other).is_some() {
                std::mem::swap(&mut x, &mut other);
            }
            let (y, c1, w) = xor_const(&q, x)?;
            if let Some((z, c2, _)) =
                binop(&q, y, BinaryOp::Or).and_then(|(z, k, _)| pattern::apint(&q, k).map(|(k, w)| (z, k, w)))
                && c2 == !c1 & mask(w)
            {
                return Some((other, BinaryOp::And, z, c1, w));
            }
            let (z, c2, _) =
                binop(&q, y, BinaryOp::And).and_then(|(z, k, _)| pattern::apint(&q, k).map(|(k, w)| (z, k, w)))?;
            (c1 == c2).then_some((other, BinaryOp::Or, z, !c1 & mask(w), w))
        });
        from_plus_one.or_else(|| {
            // C2 is odd: XOR(AND(Z, C2), C2 + 1) is NEG(OR(Z, ~C2)).
            let (l, r) = if xor_const(&q, rhs).is_some() { (rhs, lhs) } else { (lhs, rhs) };
            let (y, c1, w) = xor_const(&q, l)?;
            if c1.trailing_zeros() != 0 {
                return None;
            }
            let (z, c2, _) =
                binop(&q, y, BinaryOp::And).and_then(|(z, k, _)| pattern::apint(&q, k).map(|(k, w)| (z, k, w)))?;
            (c1 == c2.wrapping_add(1) & mask(w)).then_some((r, BinaryOp::Or, z, !c2 & mask(w), w))
        })
    };
    let (rest, op, z, k, w) = found?;
    let ty = c.type_of(z);
    debug_assert_eq!(width(c, z), w);
    let k = c.int(ty, k);
    let logic = c.create_binop(inst, op, z, k, Flags::default());
    Some(c.create_sub(inst, rest, logic))
}

/// `(A + 1) + ~B`, `(~B + A) + 1` and the other orders are `A - B` (:1690).
fn fold_add_one_not(
    c: &mut Combiner,
    inst: InstId,
    lhs: Operand,
    rhs: Operand,
) -> Option<Visit> {
    let found = {
        let q = query(c, Some(inst));
        let sum_not = |sum: Operand, not: Operand| Some((plus_one(&q, sum)?, pattern::not(&q, not)?));
        // m_c_BinOp(m_Add(A, 1), m_Not(B))
        let first = sum_not(lhs, rhs).or_else(|| sum_not(rhs, lhs));
        // m_BinOp(m_c_Add(m_Not(B), A), m_One())
        first.or_else(|| {
            pattern::one(&q, rhs).then_some(())?;
            commutative(&q, lhs, BinaryOp::Add, |x| pattern::not(&q, x).is_some())
                .and_then(|(not, a)| Some((a, pattern::not(&q, not)?)))
        })
    };
    let (a, b) = found?;
    Some(c.replace_with_binop(inst, BinaryOp::Sub, a, b, Flags::default()))
}

/// `(A + C) + ~B` is `(A - B) + (C - 1)` (:1706).
fn fold_add_const_not(
    c: &mut Combiner,
    inst: InstId,
    lhs: Operand,
    rhs: Operand,
) -> Option<Visit> {
    let found = {
        let q = query(c, Some(inst));
        let constant_add = |sum: Operand| {
            if !has_one_use(c, sum) {
                return None;
            }
            let (a, k, _) = binop(&q, sum, BinaryOp::Add)?;
            pattern::apint(&q, k).map(|(k, _)| (a, k))
        };
        // m_c_BinOp(OneUse(Add(A, C)), Not(B))
        let pair = |sum: Operand, not: Operand| Some((constant_add(sum)?, pattern::not(&q, not)?));
        // m_BinOp(OneUse(m_c_Add(Not(B), A)), C)
        let swapped = || {
            let (k, _) = pattern::apint(&q, rhs)?;
            if !has_one_use(c, lhs) {
                return None;
            }
            let (not, a) = commutative(&q, lhs, BinaryOp::Add, |x| pattern::not(&q, x).is_some())?;
            Some(((a, k), pattern::not(&q, not)?))
        };
        pair(lhs, rhs).or_else(|| pair(rhs, lhs)).or_else(swapped)
    };
    let ((a, k), b) = found?;
    let sub = c.create_sub(inst, a, b);
    let ty = c.type_of(a);
    let k = c.int(ty, k.wrapping_sub(1));
    Some(c.replace_with_binop(inst, BinaryOp::Add, sub, k, Flags::default()))
}

/// `(A + RHS) + RHS` is `A + (RHS << 1)` (:1724).
fn fold_reassociated_add(
    c: &mut Combiner,
    inst: InstId,
    lhs: Operand,
    rhs: Operand,
) -> Option<Visit> {
    for (sum, same) in [(lhs, rhs), (rhs, lhs)] {
        let a = {
            let q = query(c, Some(inst));
            if has_one_use(c, sum) { commutative(&q, sum, BinaryOp::Add, |x| x == same).map(|(_, a)| a) } else { None }
        };
        if let Some(a) = a {
            let one = c.int(c.type_of(same), 1);
            let shl = c.create_shl(inst, same, one);
            return Some(c.replace_with_binop(inst, BinaryOp::Add, a, shl, Flags::default()));
        }
    }
    None
}

/// `(A + C1) + (C2 - B)` is `(A - B) + (C1 + C2)`, and `(C1 - A) + B` is
/// `(B - A) + C1` (:1735).
fn fold_constant_sub(
    c: &mut Combiner,
    inst: InstId,
    lhs: Operand,
    rhs: Operand,
) -> Option<Visit> {
    let constant = |q: &Query, v: Operand| pattern::apint(q, v).is_some();
    let pair = {
        let q = query(c, Some(inst));
        let shape = |add: Operand, sub: Operand| {
            let (a, c1, _) = binop(&q, add, BinaryOp::Add).filter(|&(_, k, _)| constant(&q, k))?;
            let (c2, b, _) = binop(&q, sub, BinaryOp::Sub).filter(|&(k, ..)| constant(&q, k))?;
            Some((a, c1, c2, b))
        };
        shape(lhs, rhs).or_else(|| shape(rhs, lhs))
    };
    if let Some((a, c1, c2, b)) = pair
        && (has_one_use(c, lhs) || has_one_use(c, rhs))
        && let Some(sum) = fold_constants(c, BinaryOp::Add, c1, c2)
    {
        let sub = c.create_sub(inst, a, b);
        return Some(c.replace_with_binop(inst, BinaryOp::Add, sub, sum, Flags::default()));
    }
    // Canonicalize a constant sub operand as an add operand for better folding.
    let lifted = {
        let q = query(c, Some(inst));
        let shape = |sub: Operand, b: Operand| {
            has_one_use(c, sub).then_some(())?;
            let (c1, a, _) = binop(&q, sub, BinaryOp::Sub).filter(|&(k, ..)| constant(&q, k))?;
            Some((c1, a, b))
        };
        shape(lhs, rhs).or_else(|| shape(rhs, lhs))
    };
    let (c1, a, b) = lifted?;
    let sub = c.create_sub(inst, b, a);
    Some(c.replace_with_binop(inst, BinaryOp::Add, sub, c1, Flags::default()))
}

/// `(A & 2^C1) + A` is `A & (2^C1 - 1)` when bit C1 of A is a sign bit
/// (:1742).
fn fold_masked_sign_bit(
    c: &mut Combiner,
    inst: InstId,
    lhs: Operand,
    rhs: Operand,
) -> Option<Visit> {
    let (a, k, w) = {
        let q = query(c, Some(inst));
        let shape = |and: Operand, a: Operand| {
            let (x, k, _) = binop(&q, and, BinaryOp::And)?;
            let (k, w) = pattern::apint(&q, k)?;
            (x == a && k.is_power_of_two()).then_some((a, k, w))
        };
        let (a, k, w) = shape(lhs, rhs).or_else(|| shape(rhs, lhs))?;
        // countl_zero of the mask in its own width
        let leading = w - (128 - k.leading_zeros());
        (signbits::compute_num_sign_bits(&q, a, 0) > leading).then_some((a, k, w))?
    };
    let mask_c = c.int(c.type_of(a), (k - 1) & mask(w));
    Some(c.replace_with_binop(inst, BinaryOp::And, a, mask_c, Flags::default()))
}

/// `zext(B -nuw A) + zext(A)` is `zext B`; `zext(i1 A) + sext(A)` is 0
/// (:1752).
fn fold_extended_pair(
    c: &mut Combiner,
    inst: InstId,
    lhs: Operand,
    rhs: Operand,
) -> Option<Visit> {
    enum Found {
        Wide(Operand),
        Zero,
    }
    let found = {
        let q = query(c, Some(inst));
        let wide = |sub: Operand, a: Operand| {
            let (_, _, inner) = pattern::cast(&q, sub, CastOp::ZExt)?;
            let (b, a1, flags) = binop(&q, inner, BinaryOp::Sub)?;
            let (_, _, a0) = pattern::cast(&q, a, CastOp::ZExt)?;
            (a1 == a0 && flags.contains(Flags::NUW)).then_some(Found::Wide(b))
        };
        let boolean = |z: Operand, s: Operand| {
            let (_, _, a) = pattern::cast(&q, z, CastOp::ZExt)?;
            let (_, _, a1) = pattern::cast(&q, s, CastOp::SExt)?;
            (a == a1 && width(c, a) == 1).then_some(Found::Zero)
        };
        wide(lhs, rhs).or_else(|| wide(rhs, lhs)).or_else(|| boolean(lhs, rhs)).or_else(|| boolean(rhs, lhs))
    };
    let ty = c.type_of(lhs);
    match found? {
        Found::Wide(b) => Some(c.replace_with_new(inst, Opcode::Cast(CastOp::ZExt), ty, vec![b], Flags::default())),
        Found::Zero => Some(Visit::Replace(c.int(ty, 0))),
    }
}

/// `A + B` is `A | B` when A and B have no bits in common (:1775).
fn fold_no_common_bits(
    c: &mut Combiner,
    inst: InstId,
    lhs: Operand,
    rhs: Operand,
) -> Option<Visit> {
    let result = get_no_common_bits_set_result(&query(c, Some(inst)), lhs, rhs);
    let flags = match result {
        NoCommonBitsSetResult::Known => Flags::DISJOINT,
        NoCommonBitsSetResult::OnlyIfUndefIgnored => Flags::default(),
        NoCommonBitsSetResult::Unknown => return None,
    };
    Some(c.replace_with_binop(inst, BinaryOp::Or, lhs, rhs, flags))
}

/// `narrowMathIfNoOverflow` (:1683).
fn fold_narrow_math(
    c: &mut Combiner,
    inst: InstId,
    _lhs: Operand,
    _rhs: Operand,
) -> Option<Visit> {
    c.narrow_math_if_no_overflow(inst)
}

/// `(xor A, B) + (and A, B)` is `or A, B`; `(or A, B) + (and A, B)` is
/// `add A, B` in place (:1795).
fn fold_bitwise_pair(
    c: &mut Combiner,
    inst: InstId,
    lhs: Operand,
    rhs: Operand,
) -> Option<Visit> {
    let (xor, or) = {
        let q = query(c, Some(inst));
        let pair = |first: BinaryOp| {
            let shape = |logic: Operand, and: Operand| {
                let (a, b, _) = binop(&q, logic, first)?;
                is_and_of(&q, and, BinaryOp::And, a, b).then_some((a, b))
            };
            shape(lhs, rhs).or_else(|| shape(rhs, lhs))
        };
        (pair(BinaryOp::Xor), pair(BinaryOp::Or))
    };
    if let Some((a, b)) = xor {
        return Some(c.replace_with_binop(inst, BinaryOp::Or, a, b, Flags::default()));
    }
    let (a, b) = or?;
    // Replacing the operands in place preserves nuw/nsw.
    replace_operand(c, inst, 0, a);
    replace_operand(c, inst, 1, b);
    Some(Visit::Changed)
}

/// The rest of `visitAdd` this module holds, in the order `visitAdd`
/// reaches it.
pub fn fold_add_rest(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    let (lhs, rhs, flags) = parts(c, inst);
    if let Some(found) = fold_add_of_negation(c, inst, lhs, rhs, flags) {
        return found;
    }
    if let Some(value) = check_for_negative_operand(c, inst, lhs, rhs) {
        return Visit::Replace(value);
    }
    let folds: [fn(&mut Combiner, InstId, Operand, Operand) -> Option<Visit>; 9] = [
        fold_add_one_not,
        fold_add_const_not,
        fold_reassociated_add,
        fold_constant_sub,
        fold_masked_sign_bit,
        fold_extended_pair,
        fold_no_common_bits,
        fold_narrow_math,
        fold_bitwise_pair,
    ];
    folds.iter().find_map(|fold| fold(c, inst, lhs, rhs)).unwrap_or(Visit::None)
}

#[cfg(test)]
#[path = "combine_add_idioms_tests.rs"]
mod tests;
