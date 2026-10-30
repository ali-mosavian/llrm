//! `visitAnd` after the constant-mask folds: the xor/not identities and the
//! folds of an `and` with a boolean extension, which become selects.
//!
//! Translation of
//! [`visitAnd`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L2475)
//! from `and(shl(zext X, Y), SignMask)` (:2756) to the sign-bit select
//! (:2883), skipping what other families own.
//!
//! MIRROR: the C++ order, and `createSelectInstWithUnknownProfile` is a plain
//! new `select` (no `simplifySelect`), as `replace_with_new` builds it.
//!
//! DEVIATE 1: no floating-point folds (`fabs` of a bitcast) and no vectors.
//!
//! DEVIATE 2: `narrowMaskedBinOp`, `foldAndOrOfSelectUsingImpliedCond`,
//! `matchDeMorgansLaws`, `foldCastedBitwiseLogic`,
//! `foldBinopOfSextBoolToSelect` and the tail from
//! `sinkNotIntoOtherHandOfLogicalOp` on are the next strip.
//!
//! DEVIATE 3: `getFreelyInverted` (the not of a value that is not a one-use
//! `xor`) is not translated: the two `(A ^ B) & ((B ^ C) ^ A)` folds apply
//! only when the other operand has one use, where C++ builds the not directly.
//!
//! # How it works
//! Each fold matches the operands of the `and` and returns the new
//! instruction, or nothing; `fold_and_select` tries them in C++ order.
//!
//! # Used by
//! `combine_and::visit_and`.
//!
//! # Example
//! `sext(a) & b` is `a ? b : 0`:
//!
//! ```text
//! %s = sext i1 %a to i32
//! %r = and i32 %s, %b   ; %r = select i1 %a, i32 %b, i32 0
//! ```

use llrm_mir::context::mask;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::{Query, known, pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{binop, has_one_use, query, width};

/// `m_Not(P)`: the operand of the `not` of `v`.
fn not_of(
    q: &Query,
    v: Operand,
) -> Option<Operand> {
    pattern::not(q, v)
}

/// `m_c_Xor(m_Specific(a), m_Value(B))`: `B`.
fn xor_with(
    q: &Query,
    v: Operand,
    a: Operand,
) -> Option<Operand> {
    let (x, y, _) = binop(q, v, BinaryOp::Xor)?;
    if x == a {
        Some(y)
    } else if y == a {
        Some(x)
    } else {
        None
    }
}

/// `m_c_<op>(m_Specific(a), m_Specific(b))`.
fn pair_of(
    q: &Query,
    v: Operand,
    op: BinaryOp,
    a: Operand,
    b: Operand,
) -> bool {
    binop(q, v, op).is_some_and(|(x, y, _)| (x, y) == (a, b) || (x, y) == (b, a))
}

/// `m_c_Xor(m_Not(m_Value(A)), m_Value(B))`: every `(A, B)` it can bind.
fn xor_with_not(
    q: &Query,
    v: Operand,
) -> Vec<(Operand, Operand)> {
    let Some((x, y, _)) = binop(q, v, BinaryOp::Xor) else { return Vec::new() };
    [(x, y), (y, x)].into_iter().filter_map(|(n, other)| not_of(q, n).map(|a| (a, other))).collect()
}

/// `m_c_Or(m_Not(m_Value(A)), m_Value(B))`: every `(A, B)` it can bind.
fn or_with_not(
    q: &Query,
    v: Operand,
) -> Vec<(Operand, Operand)> {
    let Some((x, y, _)) = binop(q, v, BinaryOp::Or) else { return Vec::new() };
    [(x, y), (y, x)].into_iter().filter_map(|(n, other)| not_of(q, n).map(|a| (a, other))).collect()
}

/// `and(shl(zext X, Y), SignMask)` is `and(sext X, SignMask)` for the shift
/// that puts `X`'s top bit there (:2756).
fn fold_sign_mask_of_shifted_zext(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (x, ty) = {
        let q = query(c, Some(inst));
        let (bits, w) = pattern::apint(&q, op1)?;
        if bits != 1u128 << (w - 1) || !has_one_use(c, op0) {
            return None;
        }
        let (zext, y, _) = binop(&q, op0, BinaryOp::Shl)?;
        let (_, _, x) = pattern::cast(&q, zext, CastOp::ZExt)?;
        let (shift, _) = pattern::apint(&q, y)?;
        (shift == (w - width(c, x)) as u128).then_some((x, c.type_of(op0)))?
    };
    let sext = c.create_cast(inst, CastOp::SExt, x, ty, Flags::default());
    Some(c.replace_with_binop(inst, BinaryOp::And, sext, op1, Flags::default()))
}

/// The `xor`/`or`/`not` identities of an `and` (:2793).
fn fold_xor_or_not(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    enum Built {
        And(Operand, Operand),
        AndNot(Operand, Operand),
        NotAnd(Operand, Operand),
    }
    let built = {
        let q = query(c, Some(inst));
        // A & ~(A ^ B) --> A & B, and ~(A ^ B) & A --> A & B
        let absorbed = |a: Operand, other: Operand| {
            let inner = not_of(&q, other)?;
            xor_with(&q, inner, a).map(|b| Built::And(a, b))
        };
        // (A ^ B) & ((B ^ C) ^ A) --> (A ^ B) & ~C, with a one-use right side
        let xor_chain = || {
            let (a, b, _) = binop(&q, op0, BinaryOp::Xor)?;
            let (bc, a1, _) = binop(&q, op1, BinaryOp::Xor)?;
            let (b1, cc, _) = binop(&q, bc, BinaryOp::Xor)?;
            (a1 == a && b1 == b && has_one_use(c, op1)).then_some(Built::AndNot(op0, cc))
        };
        // ((A ^ C) ^ B) & (B ^ A) --> (B ^ A) & ~C, with a one-use left side
        let xor_chain_left = || {
            let (ac, b, _) = binop(&q, op0, BinaryOp::Xor)?;
            let (a, cc, _) = binop(&q, ac, BinaryOp::Xor)?;
            let (b1, a1, _) = binop(&q, op1, BinaryOp::Xor)?;
            (b1 == b && a1 == a && has_one_use(c, op0)).then_some(Built::AndNot(op1, cc))
        };
        // (A | B) & (~A ^ B) --> A & B and its mirror
        let or_xor = |or: Operand, xor: Operand| {
            xor_with_not(&q, xor)
                .into_iter()
                .find_map(|(a, b)| pair_of(&q, or, BinaryOp::Or, a, b).then_some(Built::And(a, b)))
        };
        // (~A | B) & (A ^ B) --> ~A & B and its mirror
        let not_or_xor = |or: Operand, xor: Operand| {
            or_with_not(&q, or)
                .into_iter()
                .find_map(|(a, b)| pair_of(&q, xor, BinaryOp::Xor, a, b).then_some(Built::NotAnd(a, b)))
        };
        absorbed(op0, op1)
            .or_else(|| absorbed(op1, op0))
            .or_else(xor_chain)
            .or_else(xor_chain_left)
            .or_else(|| or_xor(op0, op1))
            .or_else(|| or_xor(op1, op0))
            .or_else(|| not_or_xor(op0, op1))
            .or_else(|| not_or_xor(op1, op0))
    };
    match built? {
        Built::And(a, b) => Some(c.replace_with_binop(inst, BinaryOp::And, a, b, Flags::default())),
        Built::AndNot(keep, x) => {
            let not = c.create_not(inst, x);
            Some(c.replace_with_binop(inst, BinaryOp::And, keep, not, Flags::default()))
        }
        Built::NotAnd(a, b) => {
            let not = c.create_not(inst, a);
            Some(c.replace_with_binop(inst, BinaryOp::And, not, b, Flags::default()))
        }
    }
}

/// `select cond, t, f` of the type of `t`, as
/// `createSelectInstWithUnknownProfile`.
fn new_select(
    c: &mut Combiner,
    inst: InstId,
    cond: Operand,
    t: Operand,
    f: Operand,
) -> Visit {
    let ty = c.type_of(t);
    c.replace_with_new(inst, Opcode::Select, ty, vec![cond, t, f], Flags::default())
}

/// `m_c_And(P, m_Value(B))`: for each way the operands can bind, `(p, b)`.
fn sides(
    op0: Operand,
    op1: Operand,
) -> [(Operand, Operand); 2] {
    [(op0, op1), (op1, op0)]
}

/// `and(sext(A), B)`, `and(~sext(A), B)`, `and(zext(A), B)` with `A` a bool
/// (:2880).
fn fold_bool_extension(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let ty = c.type_of(op0);
    let zero = c.int(ty, 0);
    for (p, b) in sides(op0, op1) {
        let (sext, not_sext, zext) = {
            let q = query(c, Some(inst));
            let bool_source =
                |op: CastOp, v: Operand| pattern::cast(&q, v, op).map(|(_, _, a)| a).filter(|&a| width(c, a) == 1);
            (
                bool_source(CastOp::SExt, p),
                not_of(&q, p).and_then(|inner| bool_source(CastOp::SExt, inner)),
                if has_one_use(c, p) { bool_source(CastOp::ZExt, p) } else { None },
            )
        };
        // and(sext(A), B) --> A ? B : 0
        if let Some(a) = sext {
            return Some(new_select(c, inst, a, b, zero));
        }
        // ~sext(A) & B --> A ? 0 : B
        if let Some(a) = not_sext {
            return Some(new_select(c, inst, a, zero, b));
        }
        // and(zext(A), B) -> A ? (B & 1) : 0
        if let Some(a) = zext {
            let one = c.int(ty, 1);
            let low = c.create_binop(inst, BinaryOp::And, b, one, Flags::default());
            return Some(new_select(c, inst, a, low, zero));
        }
    }
    None
}

/// `(-1 + A) & B` is `A ? 0 : B` where `A` is 0 or 1 (:2899).
fn fold_decremented_flag(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let ty = c.type_of(op0);
    let zero = c.int(ty, 0);
    for (p, b) in sides(op0, op1) {
        let a = {
            let q = query(c, Some(inst));
            if !has_one_use(c, p) {
                continue;
            }
            let Some((x, ones, _)) = binop(&q, p, BinaryOp::Add) else { continue };
            if !pattern::all_ones(&q, ones) {
                continue;
            }
            // m_ZExtOrSelf(m_Value(A))
            pattern::cast(&q, x, CastOp::ZExt).map_or(x, |(_, _, a)| a)
        };
        if width(c, a) == 1 {
            return Some(new_select(c, inst, a, zero, b));
        }
        let max_active = {
            let q = query(c, Some(inst));
            let w = width(c, a);
            let unknown_or_one = !known::compute_known_bits(&q, a, 0).zero & mask(w);
            128 - unknown_or_one.leading_zeros()
        };
        if max_active <= 1 {
            let a_ty = c.type_of(a);
            let a_zero = c.int(a_ty, 0);
            let is_zero = c.create_icmp(inst, IntPredicate::Eq, a, a_zero);
            return Some(new_select(c, inst, is_zero, b, zero));
        }
    }
    None
}

/// `(iN X s>> (N-1)) & Y` is `(X s< 0) ? Y : 0`, with an optional `sext` or
/// `not` (:2911).
fn fold_sign_splat(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let ty = c.type_of(op0);
    let zero = c.int(ty, 0);
    for (p, y) in sides(op0, op1) {
        if !has_one_use(c, p) {
            continue;
        }
        let (x, inverted) = {
            let q = query(c, Some(inst));
            // m_SExtOrSelf(Q)
            let through_sext = |v: Operand| pattern::cast(&q, v, CastOp::SExt).map_or(v, |(_, _, a)| a);
            let ashr = |v: Operand| {
                let (x, amount, _) = binop(&q, v, BinaryOp::AShr)?;
                let (bits, _) = pattern::apint(&q, amount)?;
                (bits == (width(c, x) - 1) as u128).then_some(x)
            };
            let plain = ashr(through_sext(p)).map(|x| (x, false));
            plain
                .or_else(|| not_of(&q, through_sext(p)).and_then(&ashr).map(|x| (x, true)))
                .map_or((None, false), |(x, n)| (Some(x), n))
        };
        let Some(x) = x else { continue };
        let x_ty = c.type_of(x);
        let x_zero = c.int(x_ty, 0);
        let is_neg = c.create_icmp(inst, IntPredicate::Slt, x, x_zero);
        return Some(if inverted {
            new_select(c, inst, is_neg, zero, y)
        } else {
            new_select(c, inst, is_neg, y, zero)
        });
    }
    None
}

/// The folds of this module in the order `visitAnd` reaches them.
pub fn fold_and_select(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let steps: [fn(&mut Combiner, InstId, Operand, Operand) -> Option<Visit>; 5] =
        [fold_sign_mask_of_shifted_zext, fold_xor_or_not, fold_bool_extension, fold_decremented_flag, fold_sign_splat];
    steps.iter().find_map(|step| step(c, inst, op0, op1))
}

#[cfg(test)]
#[path = "combine_and_select_tests.rs"]
mod tests;
