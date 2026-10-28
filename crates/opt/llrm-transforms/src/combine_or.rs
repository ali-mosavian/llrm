//! `InstCombinerImpl::visitOr`: the integer `or` folds.
//!
//! Translation of
//! [`visitOr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L4196)
//! at d1106deb71cc, the folds that need only the operand patterns, in the
//! order it reaches them ([`FOLDS`]: a new one is a function and an entry).
//!
//! MIRROR: the C++ order, which operand of a commutative match is bound
//! first (a match that binds and then fails a condition does not retry the
//! other order), and the one-use conditions.
//!
//! DEVIATE 1: no vectors, and only integer constants.
//!
//! DEVIATE 2: the folds that call a helper not yet translated are left out,
//! each named where it stood: `foldOrToXor`, `foldComplexAndOrPatterns`,
//! `foldIntegerPackFromVector`, `foldOrOfInversions`, the `i1` select folds
//! (`foldAndOrOfSelectUsingImpliedCond`, `FoldOrOfLogicalAnds`),
//! `matchOrConcat`,
//! `tryFoldInstWithCtpopWithNot`, the disjoint-`or` folds, the `trunc`/`lshr`
//! mask fold, `matchSelectFromAndOr` and everything after it.
//!
//! # Example
//! `(x & 12) | (y & -13)` with `y = x | 3` keeps one mask:
//!
//! ```text
//! %o = or i32 %x, %b                           %a = and i32 %x, 12
//! %p = and i32 %o, 12                          %r = or i32 %a, %b
//! %q = and i32 %b, -13            ==>
//! %r = or i32 %p, %q
//! ```

use llrm_mir::context::mask;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags};
use llrm_mir::valuetracking::{Query, known, pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_bitorder::{fold_bitwise_logic_with_intrinsics, match_bswap_or_bitreverse};
use crate::combine_distribute::fold_using_distributive_laws;
use crate::combine_funnel::match_funnel_shift;
use crate::combine_icmp_logic::{fold_boolean_and_or, reassociate_boolean_operands};
use crate::combine_shared::{
    binop, has_one_use, other_of, parts, query, simplify_associative_or_commutative, simplify_visited, width,
};
use crate::combine_shift_binop::fold_binop_shift_with_shift;

type Fold = fn(&mut Combiner, InstId, Operand, Operand) -> Option<Visit>;

/// The rewrites in the pin's order.
const FOLDS: &[Fold] = &[
    into_select_or_phi,
    select_binop_select,
    bswap_or_bitreverse,
    match_funnel_shift,
    shift_with_shift,
    xor_constant,
    disjoint_mul,
    not_or_decrement,
    mask_merge,
    fold_bitwise_logic_with_intrinsics,
];

/// `matchBSwapOrBitReverse`.
fn bswap_or_bitreverse(
    c: &mut Combiner,
    inst: InstId,
    _: Operand,
    _: Operand,
) -> Option<Visit> {
    match_bswap_or_bitreverse(c, inst, true, true)
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

/// `foldBinOpShiftWithShift`.
fn shift_with_shift(
    c: &mut Combiner,
    inst: InstId,
    _: Operand,
    _: Operand,
) -> Option<Visit> {
    fold_binop_shift_with_shift(c, inst)
}

/// The integer constant `v` is.
fn constant(
    q: &Query,
    v: Operand,
) -> Option<u128> {
    pattern::apint(q, v).map(|(bits, _)| bits)
}

/// `(X ^ C) | Y` is `(X | Y) ^ C` when `Y & C == 0` and `C` is not all ones.
fn xor_constant(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let bits = width(c, op0);
    let (x, k, y) = {
        let q = query(c, Some(inst));
        let (x, k, y) = [(op0, op1), (op1, op0)]
            .into_iter()
            .find_map(
                |(xor, y)| {
                    if !has_one_use(c, xor) {
                        return None;
                    }
                    let (x, k, _) = binop(&q, xor, BinaryOp::Xor)?;
                    Some((x, constant(&q, k)?, y))
                },
            )?;
        (k != mask(bits) && known::masked_value_is_zero(&q, y, k, 0)).then_some((x, k, y))?
    };
    let or = c.create_binop(inst, BinaryOp::Or, x, y, Flags::default());
    let k = c.int(c.type_of(op0), k);
    Some(c.replace_with_binop(inst, BinaryOp::Xor, or, k, Flags::default()))
}

/// `or disjoint (mul X, Y), X` is `mul X, (Y + 1)`.
fn disjoint_mul(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    if !c.function.instruction(inst).flags.contains(Flags::DISJOINT) {
        return None;
    }
    let (x, y) = {
        let q = query(c, Some(inst));
        [(op0, op1), (op1, op0)]
            .into_iter()
            .find_map(
                |(mul, other)| {
                    if !has_one_use(c, mul) {
                        return None;
                    }
                    let (x, y, _) = binop(&q, mul, BinaryOp::Mul)?;
                    (x == other).then_some((x, y))
                },
            )?
    };
    let one = c.int(c.type_of(y), 1);
    let incremented = c.create_add(inst, y, one);
    Some(c.replace_with_binop(inst, BinaryOp::Mul, x, incremented, Flags::default()))
}

/// `~X | (X - 1)` is `~(X & -X)`, the form BMI lowers.
fn not_or_decrement(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let x = {
        let q = query(c, Some(inst));
        [(op0, op1), (op1, op0)]
            .into_iter()
            .find_map(
                |(not, add)| {
                    if !has_one_use(c, not) || !has_one_use(c, add) {
                        return None;
                    }
                    let x = pattern::not(&q, not)?;
                    let (first, minus_one, _) = binop(&q, add, BinaryOp::Add)?;
                    (first == x && pattern::all_ones(&q, minus_one)).then_some(x)
                },
            )?
    };
    let zero = c.int(c.type_of(x), 0);
    let negated = c.create_binop(inst, BinaryOp::Sub, zero, x, Flags::default());
    let and = c.create_binop(inst, BinaryOp::And, x, negated, Flags::default());
    let ones = c.all_ones(c.type_of(and));
    Some(c.replace_with_binop(inst, BinaryOp::Xor, and, ones, Flags::default()))
}

/// `(A & C0) | (B & C1)` for constants `C0`, `C1`.
fn mask_merge(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let bits = width(c, op0);
    let all = mask(bits);
    let ty = c.type_of(op0);
    let (a, b, c0, c1) = {
        let q = query(c, Some(inst));
        let (a, k0, _) = binop(&q, op0, BinaryOp::And)?;
        let (b, k1, _) = binop(&q, op1, BinaryOp::And)?;
        (a, b, constant(&q, k0)?, constant(&q, k1)?)
    };
    if c0 == !c1 & all {
        // ((X | B) & MaskC) | (B & ~MaskC) -> (X & MaskC) | B
        let found = {
            let q = query(c, Some(inst));
            other_of(&q, a, BinaryOp::Or, b)
                .map(|x| (x, c0, b, BinaryOp::Or))
                .or_else(|| other_of(&q, b, BinaryOp::Or, a).map(|x| (x, c1, a, BinaryOp::Or)))
                .or_else(|| other_of(&q, a, BinaryOp::Xor, b).map(|x| (x, c0, b, BinaryOp::Xor)))
                .or_else(|| other_of(&q, b, BinaryOp::Xor, a).map(|x| (x, c1, a, BinaryOp::Xor)))
        };
        if let Some((x, k, kept, op)) = found {
            let k = c.int(ty, k);
            let and = c.create_binop(inst, BinaryOp::And, x, k, Flags::default());
            return Some(c.replace_with_binop(inst, op, and, kept, Flags::default()));
        }
    }
    if c0 & c1 == 0 {
        let both = c.int(ty, c0 | c1);
        let (x_a, x_b, c2, c3) = {
            let q = query(c, Some(inst));
            // ((X | B) & C0) | (B & C1) --> (X | B) & (C0 | C1) iff X & ~C0 ==
            // 0
            let first = other_of(&q, a, BinaryOp::Or, b).filter(|&x| known::masked_value_is_zero(&q, x, !c0 & all, 0));
            // (A & C0) | ((X | A) & C1) --> (X | A) & (C0 | C1) iff X & ~C1 ==
            // 0
            let second = other_of(&q, b, BinaryOp::Or, a).filter(|&x| known::masked_value_is_zero(&q, x, !c1 & all, 0));
            // ((X | C2) & C0) | ((X | C3) & C1) --> (X | C2 | C3) & (C0 | C1)
            let bitfield = binop(&q, a, BinaryOp::Or).and_then(|(x, k2, _)| {
                let k2 = constant(&q, k2)?;
                let (y, k3, _) = binop(&q, b, BinaryOp::Or)?;
                let k3 = constant(&q, k3)?;
                (y == x && k2 & !c0 & all == 0 && k3 & !c1 & all == 0).then_some((x, k2, k3))
            });
            (first, second, bitfield.map(|f| f.0), bitfield.map(|f| (f.1, f.2)))
        };
        if x_a.is_some() {
            return Some(c.replace_with_binop(inst, BinaryOp::And, a, both, Flags::default()));
        }
        if x_b.is_some() {
            return Some(c.replace_with_binop(inst, BinaryOp::And, b, both, Flags::default()));
        }
        if let (Some(x), Some((k2, k3))) = (c2, c3) {
            let merged = c.int(ty, k2 | k3);
            let or = c.create_binop(inst, BinaryOp::Or, x, merged, Flags::default());
            return Some(c.replace_with_binop(inst, BinaryOp::And, or, both, Flags::default()));
        }
    }
    None
}

/// `InstCombinerImpl::visitOr` (:4196).
pub fn visit_or(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    // simplifyOrInst
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
    // See if we can simplify any instructions used by the instruction whose
    // sole purpose is to compute bits we don't care about.
    let found = c.simplify_demanded_instruction_bits(inst);
    if !matches!(found, Visit::None) {
        return found;
    }
    // DEVIATE 2: foldOrToXor, foldComplexAndOrPatterns,
    // foldIntegerPackFromVector, foldOrOfInversions, and the i1 select folds.
    if let Some(v) = fold_using_distributive_laws(c, inst) {
        return Visit::Replace(v);
    }
    let (op0, op1, _) = parts(c, inst);
    if let Some(found) = FOLDS.iter().find_map(|fold| fold(c, inst, op0, op1)) {
        return found;
    }
    if let Some(found) = fold_boolean_and_or(c, inst, op0, op1, false, false) {
        return Visit::Replace(found);
    }
    reassociate_boolean_operands(c, inst, (op0, op1), false).map_or(Visit::None, Visit::Replace)
}

#[cfg(test)]
#[path = "combine_or_tests.rs"]
mod tests;
