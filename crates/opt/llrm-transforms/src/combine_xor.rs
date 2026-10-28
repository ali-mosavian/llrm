//! `InstCombinerImpl::visitXor`: the integer `xor` folds.
//!
//! Translation of
//! [`visitXor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L5458)
//! at d1106deb71cc, the folds that need only the operand patterns, in the
//! order it reaches them ([`FOLDS`]: a new one is a function and an entry).
//!
//! MIRROR: the C++ order, which operand of a commutative match is bound
//! first, and the one-use conditions.
//!
//! DEVIATE 1: no vectors, and only integer constants (`ConstantExpr` and
//! `undef` lanes are not matched or made).
//!
//! DEVIATE 2: the folds that call a helper not yet translated are left out,
//! each named where it stood: `foldXorToXor`, `visitMaskedMerge`, the
//! `ctlz`/`cttz` and `fneg` bitcast folds, the `i1` select form of `(A & B) ^
//! (A | C)`, `foldXorOfICmps`, `foldCastedBitwiseLogic`, `canonicalizeAbs`,
//! `reassociateForUses`, `canonicalizeLogicFirst`, `foldLogicOfIsFPClass`,
//! `canonicalizeConditionalNegationViaMathToSelect`,
//! `foldBinOpOfDisplacedShifts`, `foldBitwiseLogicWithIntrinsics` and
//! `foldMaskedAddXorPattern`.
//!
//! DEVIATE 3: `foldNot` is in `combine_not`.
//!
//! # Example
//! `Y ^ (X | Y)` is `X & ~Y`:
//!
//! ```text
//! %o = or i32 %x, %y
//! %r = xor i32 %y, %o   ==>   %n = xor i32 %y, -1 ; %r = and i32 %x, %n
//! ```

use llrm_mir::context::mask;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::simplify::simplify_binop_at;
use llrm_mir::valuetracking::guarantee::is_guaranteed_not_to_be_undef;
use llrm_mir::valuetracking::{Query, known, pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_bitorder::fold_bitwise_logic_with_intrinsics;
use crate::combine_distribute::fold_using_distributive_laws;
use crate::combine_shared::{
    binop, has_one_use, other_of, parts, query, simplify_associative_or_commutative, simplify_visited, width,
};
use crate::combine_shift_binop::fold_binop_shift_with_shift;

type Fold = fn(&mut Combiner, InstId, Operand, Operand) -> Option<Visit>;

/// The rewrites in the pin's order.
const FOLDS: &[Fold] = &[
    xor_of_disjoint_or,
    masked_merge,
    or_constant,
    not_or_constant,
    not_and_constant,
    sign_test_select,
    sign_mask_sub,
    sign_mask_add,
    or_constant_with_clear_bits,
    inverted_shift,
    lshr_of_xor_constant,
    into_select_or_phi,
    select_binop_select,
    xor_or_common,
    xor_and_common,
    xor_xor_or,
    and_xor_pair,
    and_not_xor_not,
    not_and_xor,
    not_or_xor,
    or_or_common,
    hoist_xor_constant,
    fold_bitwise_logic_with_intrinsics,
];

/// `m_Not(v)`.
fn not_of(
    q: &Query,
    v: Operand,
) -> Option<Operand> {
    pattern::not(q, v)
}

/// `simplifyXorInst(a, b, SQ.getWithInstruction(&I))`.
fn simplify_xor(
    c: &Combiner,
    inst: InstId,
    a: Operand,
    b: Operand,
) -> Option<Operand> {
    let env = c.env;
    simplify_binop_at(c.context, env.globals, env.layout, Some(&env.analyses), c.function, inst, BinaryOp::Xor, a, b)
}

fn create_not_of(
    c: &mut Combiner,
    inst: InstId,
    v: Operand,
) -> Visit {
    let ones = c.all_ones(c.type_of(v));
    c.replace_with_binop(inst, BinaryOp::Xor, v, ones, Flags::default())
}

fn xor_with(
    c: &mut Combiner,
    inst: InstId,
    a: Operand,
    b: Operand,
) -> Option<Visit> {
    Some(c.replace_with_binop(inst, BinaryOp::Xor, a, b, Flags::default()))
}

fn and_with(
    c: &mut Combiner,
    inst: InstId,
    a: Operand,
    b: Operand,
) -> Option<Visit> {
    Some(c.replace_with_binop(inst, BinaryOp::And, a, b, Flags::default()))
}

/// `(X | Y) ^ M` with a disjoint `or` is `(X ^ M) ^ Y` where `X ^ M`
/// simplifies.
fn xor_of_disjoint_or(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (x, y, m) = {
        let q = query(c, Some(inst));
        [(op0, op1), (op1, op0)]
            .into_iter()
            .find_map(
                |(or, m)| {
                    if !has_one_use(c, or) {
                        return None;
                    }
                    pattern::disjoint_or(&q, or).map(|(x, y)| (x, y, m))
                },
            )?
    };
    if let Some(found) = simplify_xor(c, inst, x, m) {
        return xor_with(c, inst, found, y);
    }
    let found = simplify_xor(c, inst, y, m)?;
    xor_with(c, inst, found, x)
}

/// `(X & M) ^ (Y & ~M)` is `(X & M) | (Y & ~M)`.
fn masked_merge(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let m = {
        let q = query(c, Some(inst));
        [(op0, op1), (op1, op0)]
            .into_iter()
            .find_map(
                |(a, b)| {
                    let (p, r, _) = binop(&q, a, BinaryOp::And)?;
                    let m = [p, r].into_iter().find_map(|n| not_of(&q, n))?;
                    binop(&q, b, BinaryOp::And).filter(|&(s, t, _)| s == m || t == m).map(|_| m)
                },
            )?
    };
    let disjoint = is_guaranteed_not_to_be_undef(&query(c, Some(inst)), m, 0);
    let flags = if disjoint { Flags::DISJOINT } else { Flags::default() };
    Some(c.replace_with_binop(inst, BinaryOp::Or, op0, op1, flags))
}

/// The integer constant `v` is, within the width of `ty`'s operand.
fn constant(
    c: &Combiner,
    v: Operand,
) -> Option<u128> {
    query(c, None).int_constant(v).map(|(bits, _)| bits)
}

/// `(X | C2) ^ C1` is `(X & ~C2) ^ (C1 ^ C2)`.
fn or_constant(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let c1 = constant(c, op1)?;
    if !has_one_use(c, op0) {
        return None;
    }
    let (x, c2) = binop(&query(c, Some(inst)), op0, BinaryOp::Or).map(|(x, k, _)| (x, k))?;
    let c2 = constant(c, c2)?;
    let ty = c.type_of(op0);
    let m = mask(width(c, op0));
    let clear = c.int(ty, !c2 & m);
    let and = c.create_binop(inst, BinaryOp::And, x, clear, Flags::default());
    let merged = c.int(ty, (c1 ^ c2) & m);
    xor_with(c, inst, and, merged)
}

/// `(~X | C2) ^ C1` is `(X & ~C2) ^ ~C1`.
fn not_or_constant(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let c1 = constant(c, op1)?;
    if !has_one_use(c, op0) {
        return None;
    }
    let (x, c2) = {
        let q = query(c, Some(inst));
        let (n, k, _) = binop(&q, op0, BinaryOp::Or)?;
        (not_of(&q, n)?, constant(c, k)?)
    };
    let ty = c.type_of(op0);
    let m = mask(width(c, op0));
    let clear = c.int(ty, !c2 & m);
    let and = c.create_binop(inst, BinaryOp::And, x, clear, Flags::default());
    let flipped = c.int(ty, !c1 & m);
    xor_with(c, inst, and, flipped)
}

/// `(~X & C2) ^ C1` is `(X | ~C2) ^ ~C1`.
fn not_and_constant(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let c1 = constant(c, op1)?;
    if !has_one_use(c, op0) {
        return None;
    }
    let (x, c2) = {
        let q = query(c, Some(inst));
        let (n, k, _) = binop(&q, op0, BinaryOp::And)?;
        (not_of(&q, n)?, constant(c, k)?)
    };
    let ty = c.type_of(op0);
    let m = mask(width(c, op0));
    let set = c.int(ty, !c2 & m);
    let or = c.create_binop(inst, BinaryOp::Or, x, set, Flags::default());
    let flipped = c.int(ty, !c1 & m);
    xor_with(c, inst, or, flipped)
}

/// `xor ([trunc] (ashr X, BW-1)), C` is `select (X >s -1), C, ~C`.
fn sign_test_select(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let c1 = constant(c, op1)?;
    if !has_one_use(c, op0) {
        return None;
    }
    let x = {
        let q = query(c, Some(inst));
        let shifted = match c.definition_of(op0).map(|d| c.function.instruction(d)) {
            Some(i) if i.opcode == Opcode::Cast(CastOp::Trunc) => i.operands[0],
            _ => op0,
        };
        let (x, amount, _) = binop(&q, shifted, BinaryOp::AShr)?;
        let (amount, _) = pattern::apint(&q, amount)?;
        (amount == u128::from(width(c, x)) - 1 && c1 != mask(width(c, op1))).then_some(x)?
    };
    let zero_to_ones = c.int(c.type_of(x), u128::MAX);
    let is_not_neg = c.create_icmp(inst, IntPredicate::Sgt, x, zero_to_ones);
    let ty = c.type_of(op1);
    let flipped = c.int(ty, !c1 & mask(width(c, op1)));
    Some(c.replace_with_new(inst, Opcode::Select, ty, vec![is_not_neg, op1, flipped], Flags::default()))
}

/// `(C - X) ^ signmask` is `(C + signmask) - X`.
fn sign_mask_sub(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (k, x, rhs) = {
        let q = query(c, Some(inst));
        if !pattern::sign_mask(&q, op1) {
            return None;
        }
        let (k, x, _) = binop(&q, op0, BinaryOp::Sub)?;
        (constant(c, k)?, x, constant(c, op1)?)
    };
    let sum = c.int(c.type_of(op0), k.wrapping_add(rhs) & mask(width(c, op0)));
    Some(c.replace_with_binop(inst, BinaryOp::Sub, sum, x, Flags::default()))
}

/// `(X + C) ^ signmask` is `X + (C + signmask)`.
fn sign_mask_add(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (x, k, rhs) = {
        let q = query(c, Some(inst));
        if !pattern::sign_mask(&q, op1) {
            return None;
        }
        let (x, k, _) = binop(&q, op0, BinaryOp::Add)?;
        (x, constant(c, k)?, constant(c, op1)?)
    };
    let sum = c.int(c.type_of(op0), k.wrapping_add(rhs) & mask(width(c, op0)));
    Some(c.replace_with_binop(inst, BinaryOp::Add, x, sum, Flags::default()))
}

/// `(X | C) ^ RHSC` is `X ^ (C ^ RHSC)` when `X & C == 0`.
fn or_constant_with_clear_bits(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let rhs = constant(c, op1)?;
    let (x, k) = {
        let q = query(c, Some(inst));
        let (x, k, _) = binop(&q, op0, BinaryOp::Or)?;
        let k = constant(c, k)?;
        known::masked_value_is_zero(&q, x, k, 0).then_some((x, k))?
    };
    let folded = c.int(c.type_of(op0), (k ^ rhs) & mask(width(c, op0)));
    xor_with(c, inst, x, folded)
}

/// `(X << C) ^ (-1 << C)` is `~X << C`, and `(X >>u C) ^ (-1 >>u C)` is
/// `~X >>u C`.
fn inverted_shift(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let rhs = constant(c, op1)?;
    if !has_one_use(c, op0) {
        return None;
    }
    let bits = width(c, op0);
    let (op, x, amount) = {
        let q = query(c, Some(inst));
        [BinaryOp::Shl, BinaryOp::LShr]
            .into_iter()
            .find_map(
                |op| {
                    let (x, amount, _) = binop(&q, op0, op)?;
                    let (amount, _) = pattern::apint(&q, amount)?;
                    if amount >= u128::from(bits) {
                        return None;
                    }
                    let ones = mask(bits);
                    let wanted = if op == BinaryOp::Shl { (ones << amount) & ones } else { ones >> amount };
                    (rhs == wanted).then_some((op, x, amount))
                },
            )?
    };
    let not = c.create_not(inst, x);
    let by = c.int(c.type_of(op0), amount);
    Some(c.replace_with_binop(inst, op, not, by, Flags::default()))
}

/// `((X ^ C1) >> C2) ^ C3` is `(X >> C2) ^ ((C1 >> C2) ^ C3)`.
fn lshr_of_xor_constant(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let c3 = constant(c, op1)?;
    if !has_one_use(c, op0) {
        return None;
    }
    let (x, c1, c2) = {
        let q = query(c, Some(inst));
        let (inner, c2, _) = binop(&q, op0, BinaryOp::LShr)?;
        let (x, c1, _) = binop(&q, inner, BinaryOp::Xor)?;
        (x, constant(c, c1)?, constant(c, c2)?)
    };
    if c2 >= u128::from(width(c, op0)) {
        return None;
    }
    let ty = c.type_of(op0);
    let by = c.int(ty, c2);
    let shifted = c.create_binop(inst, BinaryOp::LShr, x, by, Flags::default());
    let folded = c.int(ty, ((c1 >> c2) ^ c3) & mask(width(c, op0)));
    xor_with(c, inst, shifted, folded)
}

fn into_select_or_phi(
    c: &mut Combiner,
    inst: InstId,
    _: Operand,
    _: Operand,
) -> Option<Visit> {
    Some(c.fold_bin_op_into_select_or_phi(inst)).filter(|found| !matches!(found, Visit::None))
}

fn select_binop_select(
    c: &mut Combiner,
    inst: InstId,
    _: Operand,
    _: Operand,
) -> Option<Visit> {
    Some(c.fold_bin_op_select_bin_op(inst)).filter(|found| !matches!(found, Visit::None))
}

/// `Y ^ (X | Y)` and `(X | Y) ^ Y` are `X & ~Y`.
fn xor_or_common(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (x, y) = {
        let q = query(c, Some(inst));
        [(op1, op0), (op0, op1)]
            .into_iter()
            .find_map(
                |(or, y)| {
                    if !has_one_use(c, or) {
                        return None;
                    }
                    other_of(&q, or, BinaryOp::Or, y).map(|x| (x, y))
                },
            )?
    };
    let not = c.create_not(inst, y);
    and_with(c, inst, x, not)
}

/// `Y ^ (X & Y)` and `(X & Y) ^ Y` are `~X & Y`; the latter unless `Y` is a
/// constant, whose `(X & C) ^ C` is the canonical form.
fn xor_and_common(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (x, y) = {
        let q = query(c, Some(inst));
        let first = has_one_use(c, op1).then(|| other_of(&q, op1, BinaryOp::And, op0)).flatten().map(|x| (x, op0));
        first.or_else(|| {
            if matches!(op1, Operand::Constant(_)) || !has_one_use(c, op0) {
                return None;
            }
            other_of(&q, op0, BinaryOp::And, op1).map(|x| (x, op1))
        })?
    };
    let not = c.create_not(inst, x);
    and_with(c, inst, y, not)
}

/// `(A ^ B) ^ (A | C)` is `(~A & C) ^ B`, and `(A ^ B) ^ (B | C)` is
/// `(~B & C) ^ A`; each in the four commuted variants.
fn xor_xor_or(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    for first in [true, false] {
        let found = {
            let q = query(c, Some(inst));
            [(op0, op1), (op1, op0)]
                .into_iter()
                .find_map(
                    |(xor, or)| {
                        if !has_one_use(c, xor) || !has_one_use(c, or) {
                            return None;
                        }
                        let (a, b, _) = binop(&q, xor, BinaryOp::Xor)?;
                        let (shared, kept) = if first { (a, b) } else { (b, a) };
                        other_of(&q, or, BinaryOp::Or, shared).map(|other| (shared, other, kept))
                    },
                )
        };
        if let Some((shared, other, kept)) = found {
            let not = c.create_not(inst, shared);
            let and = c.create_binop(inst, BinaryOp::And, not, other, Flags::default());
            return xor_with(c, inst, and, kept);
        }
    }
    None
}

/// `(A & B) ^ (A ^ B)` and `(A ^ B) ^ (A & B)` are `A | B`.
fn and_xor_pair(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (a, b) = {
        let q = query(c, Some(inst));
        let pair = |v: Operand, op: BinaryOp, a: Operand, b: Operand| {
            binop(&q, v, op).is_some_and(|(x, y, _)| (x, y) == (a, b) || (x, y) == (b, a))
        };
        binop(&q, op0, BinaryOp::And)
            .filter(|&(a, b, _)| pair(op1, BinaryOp::Xor, a, b))
            .or_else(|| binop(&q, op0, BinaryOp::Xor).filter(|&(a, b, _)| pair(op1, BinaryOp::And, a, b)))
            .map(|(a, b, _)| (a, b))?
    };
    Some(c.replace_with_binop(inst, BinaryOp::Or, a, b, Flags::default()))
}

/// `(A & ~B) ^ ~A` is `~(A & B)`.
fn and_not_xor_not(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (a, b) = {
        let q = query(c, Some(inst));
        let (p, r, _) = binop(&q, op0, BinaryOp::And)?;
        let not_a = not_of(&q, op1)?;
        [(p, r), (r, p)].into_iter().find_map(|(a, n)| (a == not_a).then(|| not_of(&q, n).map(|b| (a, b))).flatten())?
    };
    let and = c.create_binop(inst, BinaryOp::And, a, b, Flags::default());
    Some(create_not_of(c, inst, and))
}

/// `(~A & B) ^ A` is `A | B`, in the four commuted variants.
fn not_and_xor(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (a, b) = {
        let q = query(c, Some(inst));
        [(op0, op1), (op1, op0)]
            .into_iter()
            .find_map(
                |(and, a)| {
                    let (p, r, _) = binop(&q, and, BinaryOp::And)?;
                    [(p, r), (r, p)].into_iter().find_map(|(n, b)| (not_of(&q, n) == Some(a)).then_some((a, b)))
                },
            )?
    };
    Some(c.replace_with_binop(inst, BinaryOp::Or, a, b, Flags::default()))
}

/// `(~A | B) ^ A` and `A ^ (~A | B)` are `~(A & B)`.
fn not_or_xor(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (a, b) = {
        let q = query(c, Some(inst));
        [(op0, op1), (op1, op0)]
            .into_iter()
            .find_map(
                |(or, a)| {
                    if !has_one_use(c, or) {
                        return None;
                    }
                    let (p, r, _) = binop(&q, or, BinaryOp::Or)?;
                    [(p, r), (r, p)].into_iter().find_map(|(n, b)| (not_of(&q, n) == Some(a)).then_some((a, b)))
                },
            )?
    };
    let and = c.create_binop(inst, BinaryOp::And, a, b, Flags::default());
    Some(create_not_of(c, inst, and))
}

/// `(A | B) ^ (A | C)` is `(B ^ C) & ~A`, in the four commuted variants.
fn or_or_common(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    if !has_one_use(c, op0) || !has_one_use(c, op1) {
        return None;
    }
    let (mut a, mut b, mut cc, mut d) = {
        let q = query(c, Some(inst));
        let (a, b, _) = binop(&q, op0, BinaryOp::Or)?;
        let (cc, d, _) = binop(&q, op1, BinaryOp::Or)?;
        (a, b, cc, d)
    };
    if b == cc || b == d {
        std::mem::swap(&mut a, &mut b);
    }
    if a == cc {
        std::mem::swap(&mut cc, &mut d);
    }
    if a != d {
        return None;
    }
    let not = c.create_not(inst, a);
    let xor = c.create_binop(inst, BinaryOp::Xor, b, cc, Flags::default());
    and_with(c, inst, xor, not)
}

/// `(X ^ C1) ^ Y` is `(X ^ Y) ^ C1`: the constant moves out last.
fn hoist_xor_constant(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (x, k, y) = {
        let q = query(c, Some(inst));
        [(op0, op1), (op1, op0)]
            .into_iter()
            .find_map(
                |(xor, y)| {
                    if !has_one_use(c, xor) {
                        return None;
                    }
                    let (x, k, _) = binop(&q, xor, BinaryOp::Xor)?;
                    // m_Unless(m_ConstantExpr()) on X, m_ImmConstant on C1.
                    let is_expr = matches!(x, Operand::Constant(_)) && !pattern::imm_constant(&q, x);
                    (pattern::imm_constant(&q, k) && !is_expr).then_some((x, k, y))
                },
            )?
    };
    let xor = c.create_binop(inst, BinaryOp::Xor, x, y, Flags::default());
    xor_with(c, inst, xor, k)
}

/// `InstCombinerImpl::visitXor` (:5458).
pub fn visit_xor(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    // simplifyXorInst
    if let Some(found) = simplify_visited(c, inst) {
        return found;
    }
    if simplify_associative_or_commutative(c, inst) {
        return Visit::Changed;
    }
    // DEVIATE 1: foldVectorBinop.
    let found = c.fold_binop_with_phi_operands(inst);
    if !matches!(found, Visit::None) {
        return found;
    }
    // DEVIATE 2: foldXorToXor.
    if let Some(found) = crate::combine_not::fold_not(c, inst) {
        return found;
    }
    if let Some(v) = fold_using_distributive_laws(c, inst) {
        return Visit::Replace(v);
    }
    // See if we can simplify any instructions used by the instruction whose
    // sole purpose is to compute bits we don't care about.
    let found = c.simplify_demanded_instruction_bits(inst);
    if !matches!(found, Visit::None) {
        return found;
    }
    if let Some(r) = fold_binop_shift_with_shift(c, inst) {
        return r;
    }
    let (op0, op1, _) = parts(c, inst);
    FOLDS.iter().find_map(|fold| fold(c, inst, op0, op1)).unwrap_or(Visit::None)
}

#[cfg(test)]
#[path = "combine_xor_tests.rs"]
mod tests;
