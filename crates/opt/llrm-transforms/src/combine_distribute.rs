//! `InstCombinerImpl::foldUsingDistributiveLaws`: factor a common term out of
//! two operands of a binary operator, or expand an operator over an operand
//! it distributes over when both halves simplify.
//!
//! Translation of
//! [`foldUsingDistributiveLaws`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L1200),
//! [`tryFactorizationFolds`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L1157),
//! [`tryFactorization`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L750)
//! and the helpers they read (`leftDistributesOverRight`,
//! `rightDistributesOverLeft`, `getIdentityValue`,
//! `getBinOpsForFactorization`) in the order the C++ reaches them, ending in
//! `SimplifySelectsFeedingBinaryOp`.
//!
//! MIRROR: factor first (`(A op' B) op (A op' D)` to `A op' (B op D)`, and the
//! `(A op' B) op C` forms with the identity of `op'` for the missing arm, a
//! `shl` by a constant read as the `mul` of `1 << C` under `add` and `sub`),
//! then expand `(A op' B) op C` and `A op (B op' C)` when both halves
//! simplify (without `undef`) or one half is the identity of `op'`.
//!
//! DEVIATE 1: `getBinOpsForFactorization` keeps the `shl` as it is where
//! `1 << C` does not fold to a constant (the C++ asserts it does).
//!
//! DEVIATE 2: the `nsw`/`nuw` of a factored `add` of `mul`s are set on the
//! result only when `tryFactorization` made it; the C++ also sets them on an
//! existing binary operator the builder's folder returned.
//!
//! # Used by
//! `visit_and`, `visit_or`, `visit_xor`, `visit_add`, `visit_sub`, `visit_mul`.
//!
//! # Example
//! ```text
//! %a = mul i32 %x, %y
//! %b = mul i32 %x, %z                ==>   %s = add i32 %y, %z
//! %r = add i32 %a, %b                      %r = mul i32 %x, %s
//! ```

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::simplify::simplify_binop_at_without_undef;
use llrm_mir::valuetracking::pattern;

use crate::combine::Combiner;
use crate::combine_shared::{inst_def, is_commutative, parts, query, set_flag, simplify_binop};

/// `Instruction::isBitwiseLogicOp`.
fn is_bitwise_logic(op: BinaryOp) -> bool {
    matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)
}

/// `Instruction::isShift`.
fn is_shift(op: BinaryOp) -> bool {
    matches!(op, BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr)
}

/// `isa<OverflowingBinaryOperator>`.
fn is_overflowing(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl
    )
}

/// Whether `x lop (y rop z)` is always `(x lop y) rop (x lop z)`.
fn left_distributes_over_right(
    lop: BinaryOp,
    rop: BinaryOp,
) -> bool {
    match lop {
        BinaryOp::And => matches!(rop, BinaryOp::Or | BinaryOp::Xor),
        BinaryOp::Or => rop == BinaryOp::And,
        BinaryOp::Mul => matches!(rop, BinaryOp::Add | BinaryOp::Sub),
        _ => false,
    }
}

/// Whether `(x lop y) rop z` is always `(x rop z) lop (y rop z)`.
fn right_distributes_over_left(
    lop: BinaryOp,
    rop: BinaryOp,
) -> bool {
    if is_commutative(rop) {
        return left_distributes_over_right(rop, lop);
    }
    // (X {&|^} Y) >> Z <--> (X >> Z) {&|^} (Y >> Z) for all shifts.
    is_bitwise_logic(lop) && is_shift(rop)
}

/// `ConstantExpr::getBinOpIdentity(op, ty)` for the integer operators that
/// have one with no constant on the right: `x op identity` is `x`.
fn bin_op_identity(
    c: &Combiner,
    op: BinaryOp,
    ty: llrm_mir::types::TypeId,
) -> Option<Operand> {
    match op {
        BinaryOp::Add | BinaryOp::Or | BinaryOp::Xor => Some(c.int(ty, 0)),
        BinaryOp::Mul => Some(c.int(ty, 1)),
        BinaryOp::And => Some(c.all_ones(ty)),
        _ => None,
    }
}

/// `getIdentityValue(op, v)`: none for a constant.
fn identity_value(
    c: &Combiner,
    op: BinaryOp,
    v: Operand,
) -> Option<Operand> {
    if matches!(v, Operand::Constant(_)) {
        return None;
    }
    bin_op_identity(c, op, c.type_of(v))
}

/// The defining instruction of `v` if it is a binary operator: its id, opcode
/// and operands.
fn binary(
    c: &Combiner,
    v: Operand,
) -> Option<(InstId, BinaryOp, Operand, Operand)> {
    let def = inst_def(c, v)?;
    let instruction = c.function.instruction(def);
    let Opcode::Binary(op) = instruction.opcode else { return None };
    Some((def, op, instruction.operands[0], instruction.operands[1]))
}

/// `getBinOpsForFactorization(top, op, lhs, rhs, other)`: the operator and
/// operands `op` is factored as; `other` is the opcode of the other operand.
fn binops_for_factorization(
    c: &Combiner,
    top: BinaryOp,
    (_, op, lhs, rhs): (InstId, BinaryOp, Operand, Operand),
    other: Option<BinaryOp>,
) -> (BinaryOp, Operand, Operand) {
    let q = query(c, None);
    if matches!(top, BinaryOp::Add | BinaryOp::Sub) && op == BinaryOp::Shl && pattern::imm_constant(&q, rhs) {
        // X << C --> X * (1 << C)
        let one = c.int(c.type_of(lhs), 1);
        if let Some(scaled) = crate::combine_shared::fold_constants(c, BinaryOp::Shl, one, rhs) {
            return (BinaryOp::Mul, lhs, scaled);
        }
    }
    if is_bitwise_logic(top) && other == Some(BinaryOp::AShr) && op == BinaryOp::LShr {
        // lshr nneg C, X --> ashr nneg C, X
        if q.int_constant(lhs).is_some_and(|(bits, width)| bits >> (width - 1) & 1 == 0) {
            return (BinaryOp::AShr, lhs, rhs);
        }
    }
    (op, lhs, rhs)
}

/// `tryFactorization(I, SQ, Builder, inner, A, B, C, D)`: `inst`, the form
/// `(A inner B) top (C inner D)`, as `A inner (B top D)` or `(A top C) inner
/// B`.
fn try_factorization(
    c: &mut Combiner,
    inst: InstId,
    inner: BinaryOp,
    a: Operand,
    b: Operand,
    mut cc: Operand,
    mut d: Operand,
) -> Option<Operand> {
    let Opcode::Binary(top) = c.function.instruction(inst).opcode else { return None };
    let (lhs, rhs, flags) = parts(c, inst);
    let first_id = c.function.instruction_count();
    let inner_commutative = is_commutative(inner);
    let mut v = None;
    let mut found = None;

    // Does "X op' (Y op Z)" always equal "(X op' Y) op (X op' Z)"?
    if left_distributes_over_right(inner, top) && (a == cc || (inner_commutative && a == d)) {
        if a != cc {
            std::mem::swap(&mut cc, &mut d);
        }
        // Consider forming "A op' (B op D)". If "B op D" simplifies then it
        // can be formed with no cost; else only if an existing operation
        // "A op' B" or "C op' D" will be zapped as no longer used.
        v = simplify_binop(c, inst, top, b, d);
        if v.is_none() && (c.has_one_use(lhs) || c.has_one_use(rhs)) {
            v = Some(c.create_binop(inst, top, b, d, Flags::default()));
        }
        if let Some(v) = v {
            found = Some(c.create_binop(inst, inner, a, v, Flags::default()));
        }
    }

    // Does "(X op Y) op' Z" always equal "(X op' Z) op (Y op' Z)"?
    if found.is_none() && right_distributes_over_left(top, inner) && (b == d || (inner_commutative && b == cc)) {
        if b != d {
            std::mem::swap(&mut cc, &mut d);
        }
        // Consider forming "(A op C) op' B".
        v = simplify_binop(c, inst, top, a, cc);
        if v.is_none() && (c.has_one_use(lhs) || c.has_one_use(rhs)) {
            v = Some(c.create_binop(inst, top, a, cc, Flags::default()));
        }
        if let Some(v) = v {
            found = Some(c.create_binop(inst, inner, v, b, Flags::default()));
        }
    }
    let found = found?;

    // Try to add no-overflow flags to the final value.
    let made = inst_def(c, found).filter(|def| def.0 as usize >= first_id);
    if let Some(made) = made.filter(|&def| matches!(c.function.instruction(def).opcode, Opcode::Binary(_))) {
        let (mut nsw, mut nuw) =
            if is_overflowing(top) { (flags.contains(Flags::NSW), flags.contains(Flags::NUW)) } else { (false, false) };
        for side in [lhs, rhs] {
            if let Some((def, op, ..)) = binary(c, side).filter(|&(_, op, ..)| is_overflowing(op)) {
                let _ = op;
                let side_flags = c.function.instruction(def).flags;
                nsw &= side_flags.contains(Flags::NSW);
                nuw &= side_flags.contains(Flags::NUW);
            }
        }
        if top == BinaryOp::Add && inner == BinaryOp::Mul {
            // We can propagate 'nsw' if we know that
            //  %Y = mul nsw i16 %X, C
            //  %Z = add nsw i16 %Y, %X
            // =>
            //  %Z = mul nsw i16 %X, C+1
            // iff C+1 isn't INT_MIN.
            let q = query(c, None);
            if let Some((bits, width)) = v.and_then(|v| q.int_constant(v))
                && bits != 1u128 << (width - 1)
            {
                set_flag(c, made, Flags::NSW, nsw);
            }
            // nuw can be propagated with any constant or nuw value.
            set_flag(c, made, Flags::NUW, nuw);
        }
    }
    Some(found)
}

/// `tryFactorizationFolds(I)`: a common term factored out of the operands.
fn try_factorization_folds(
    c: &mut Combiner,
    inst: InstId,
    top: BinaryOp,
    lhs: Operand,
    rhs: Operand,
) -> Option<Operand> {
    let op0 = binary(c, lhs);
    let op1 = binary(c, rhs);
    let left = op0.map(|op| binops_for_factorization(c, top, op, op1.map(|other| other.1)));
    let right = op1.map(|op| binops_for_factorization(c, top, op, op0.map(|other| other.1)));

    // The instruction has the form "(A op' B) op (C op' D)". Try to factorize
    // a common term.
    if let (Some((lop, a, b)), Some((rop, cc, d))) = (left, right)
        && lop == rop
        && let Some(found) = try_factorization(c, inst, lop, a, b, cc, d)
    {
        return Some(found);
    }
    // The instruction has the form "(A op' B) op (C)". Try to factorize common
    // term.
    if let Some((lop, a, b)) = left
        && let Some(ident) = identity_value(c, lop, rhs)
        && let Some(found) = try_factorization(c, inst, lop, a, b, rhs, ident)
    {
        return Some(found);
    }
    // The instruction has the form "(B) op (C op' D)". Try to factorize common
    // term.
    if let Some((rop, cc, d)) = right
        && let Some(ident) = identity_value(c, rop, lhs)
        && let Some(found) = try_factorization(c, inst, rop, lhs, ident, cc, d)
    {
        return Some(found);
    }
    None
}

/// `foldUsingDistributiveLaws(I)`: the value `inst`, a binary operator, equals
/// after factoring or expanding, if it simplifies.
pub fn fold_using_distributive_laws(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Operand> {
    let Opcode::Binary(top) = c.function.instruction(inst).opcode else { return None };
    let (lhs, rhs, _) = parts(c, inst);

    // Factorization.
    if let Some(found) = try_factorization_folds(c, inst, top, lhs, rhs) {
        return Some(found);
    }

    // Disable the use of undef because it's not safe to distribute undef.
    let simplify = |c: &Combiner, a: Operand, b: Operand| {
        let env = c.env;
        simplify_binop_at_without_undef(
            c.context,
            env.globals,
            env.layout,
            Some(&env.analyses),
            c.function,
            inst,
            top,
            a,
            b,
        )
    };

    // Expansion.
    if let Some((_, inner, a, b)) = binary(c, lhs).filter(|&(_, inner, ..)| right_distributes_over_left(inner, top)) {
        // The instruction has the form "(A op' B) op C". See if expanding it
        // out to "(A op C) op' (B op C)" results in simplifications.
        let (l, r) = (simplify(c, a, rhs), simplify(c, b, rhs));
        let identity =
            |c: &Combiner, v: Option<Operand>| v.is_some_and(|v| bin_op_identity(c, inner, c.type_of(v)) == Some(v));
        // Do "A op C" and "B op C" both simplify?
        if let (Some(l), Some(r)) = (l, r) {
            return Some(c.create_binop(inst, inner, l, r, Flags::default()));
        }
        // Does "A op C" simplify to the identity value for the inner opcode?
        if identity(c, l) {
            return Some(c.create_binop(inst, top, b, rhs, Flags::default()));
        }
        // Does "B op C" simplify to the identity value for the inner opcode?
        if identity(c, r) {
            return Some(c.create_binop(inst, top, a, rhs, Flags::default()));
        }
    }

    if let Some((_, inner, b, cc)) = binary(c, rhs).filter(|&(_, inner, ..)| left_distributes_over_right(top, inner)) {
        // The instruction has the form "A op (B op' C)". See if expanding it
        // out to "(A op B) op' (A op C)" results in simplifications.
        let (l, r) = (simplify(c, lhs, b), simplify(c, lhs, cc));
        let identity =
            |c: &Combiner, v: Option<Operand>| v.is_some_and(|v| bin_op_identity(c, inner, c.type_of(v)) == Some(v));
        if let (Some(l), Some(r)) = (l, r) {
            return Some(c.create_binop(inst, inner, l, r, Flags::default()));
        }
        // Does "A op B" simplify to the identity value for the inner opcode?
        if identity(c, l) {
            return Some(c.create_binop(inst, top, lhs, cc, Flags::default()));
        }
        // Does "A op C" simplify to the identity value for the inner opcode?
        if identity(c, r) {
            return Some(c.create_binop(inst, top, lhs, b, Flags::default()));
        }
    }

    c.simplify_selects_feeding_binary_op(inst, lhs, rhs)
}

#[cfg(test)]
#[path = "combine_distribute_tests.rs"]
mod tests;
