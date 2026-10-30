//! `foldSelectIntoOp`: `select c, (op x, y), x` as `op x, (select c, y,
//! identity)`.
//!
//! Translation of
//! [`InstCombinerImpl::foldSelectIntoOp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L559)
//! at d1106deb71cc, with `getSelectFoldableOperands` (:251): the select picks
//! only the operand that differs, and the operation runs on either path. The
//! shape `if (x >= r + bit) { x -= r + bit; ... }` makes
//! `select c, (sub x, y), x`; the operation then runs after a select of `y` or
//! zero, not before a select of its result.
//!
//! MIRROR: the operation has one use; the other arm is no constant; it is
//! the operation's first operand (any of them for a commutative operation); the
//! operation's other operand, if a constant, must make a select of 0, 1 or -1
//! with the identity; the new operation keeps the old one's flags.
//!
//! DEVIATE 1: only integer operations. The floating ones (`fadd`, `fsub`,
//! `fmul`, `fdiv`) need the select's fast-math flags and `isKnownNeverNaN` of
//! the other arm, which `computeKnownFPClass` answers where #2209 lands.
//!
//! DEVIATE 2: the new operation is not named after the old one (`takeName`):
//! MIR has no rename.
//!
//! # Example
//! ```text
//! %d = sub i32 %x, %y                      %s = select i1 %c, i32 %y, i32 0
//! %r = select i1 %c, i32 %d, i32 %x   ==>  %r = sub i32 %x, %s
//! ```

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{has_one_use, query};
use crate::combine_visit_select::Sel;

/// `getSelectFoldableOperands`: which operands of an operation the select can
/// reach the identity through, by the operand the other arm is. `[first,
/// second]`: the other arm may be the first / second operand.
fn foldable_operands(op: BinaryOp) -> [bool; 2] {
    match op {
        // Either operand.
        BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor => [true, true],
        // The amount subtracted, or shifted by, only.
        BinaryOp::Sub | BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr => [true, false],
        _ => [false, false],
    }
}

/// `isSelect01`: one of the two is zero and the other is 1 or -1.
fn is_select_01(
    a: u128,
    b: u128,
    width: u32,
) -> bool {
    let all_ones = llrm_mir::context::mask(width);
    if a != 0 && b != 0 {
        return false;
    }
    [a, b].iter().any(|&one| one == 1 || one == all_ones)
}

/// `foldSelectIntoOp(SI, TrueVal, FalseVal)` for an integer select.
pub(crate) fn select_into_op(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    if !matches!(c.context.types.get(sel.ty), llrm_mir::types::Type::Int(_)) {
        return None;
    }
    fold(c, inst, sel, sel.if_true, sel.if_false, false).or_else(|| fold(c, inst, sel, sel.if_false, sel.if_true, true))
}

/// `TryFoldSelectIntoOp`: `operation` is the arm that is an operation, `other`
/// the arm it must read; `swapped` when `operation` is the false arm.
fn fold(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
    operation: Operand,
    other: Operand,
    swapped: bool,
) -> Option<Visit> {
    let tvi = c.definition_of(operation)?;
    let Opcode::Binary(op) = c.function.instruction(tvi).opcode else { return None };
    let [first, second] = c.function.instruction(tvi).operands[..] else { return None };
    let flags = c.function.instruction(tvi).flags;
    if !has_one_use(c, operation) || matches!(other, Operand::Constant(_)) {
        return None;
    }

    let [by_first, by_second] = foldable_operands(op);
    let folded_operand = if by_first && other == first {
        second
    } else if by_second && other == second {
        first
    } else {
        return None;
    };
    let identity = c.bin_op_identity(op, sel.ty, true)?;
    // No select between two constants, unless of 0, 1 and -1.
    if let Operand::Constant(_) = folded_operand {
        let q = query(c, Some(inst));
        let (value, width) = q.int_constant(folded_operand)?;
        let (identity_value, _) = q.int_constant(identity)?;
        if !is_select_01(identity_value, value, width) {
            return None;
        }
    }

    let arms = if swapped { [identity, folded_operand] } else { [folded_operand, identity] };
    let select = c.select_like(inst, inst, [sel.cond, arms[0], arms[1]], Flags::default());
    Some(c.replace_with_new(inst, Opcode::Binary(op), sel.ty, vec![other, select], flags))
}

#[cfg(test)]
#[path = "combine_select_into_op_tests.rs"]
mod tests;
