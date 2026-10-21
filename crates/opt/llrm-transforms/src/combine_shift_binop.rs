//! `InstCombinerImpl::foldBinOpShiftWithShift`: a shift of each arm of a
//! bitwise operator or `add` is one shift of the operator.
//!
//! Translation of
//! [`foldBinOpShiftWithShift`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L959)
//! at d1106deb71cc.
//!
//! MIRROR: `(binop1 (binop2 (shift X, S), Mask), (shift Y, S))`, both shifts
//! logical and by the same amount, becomes `binop2 (shift (binop1 X, Y), S),
//! Mask` where the operators are the same (and not an `add` under a right
//! shift), else `shift (binop1 Y, (binop2 X, Mask')), S` with the mask shifted
//! back by a constant amount where that loses no bit; the arithmetic-shift
//! case is `(Y >>s S) op ~(X >>s S)`.
//!
//! DEVIATE 1: no vectors, as everywhere in MIR.
//!
//! # Used by
//! `visit_and`, `visit_or`, `visit_xor`, `visit_add`.
//!
//! # Example
//! ```text
//! %a = shl i32 %x, 4
//! %b = and i32 %a, 240                          %c = and i32 %x, 15
//! %d = shl i32 %y, 4                    ==>     %e = and i32 %y, ...  (see tests)
//! %r = or i32 %b, %d
//! ```

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{fold_constants, query};

/// `IsValidBinOpc`: skip `sub`, as only constant masks are matched, which
/// canonicalize to `add`.
fn valid_binop(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::And | BinaryOp::Or | BinaryOp::Xor | BinaryOp::Add
    )
}

fn is_shift(op: BinaryOp) -> bool {
    matches!(op, BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr)
}

/// `IsCompletelyDistributable`: `add` distributes over `shl` alone.
fn completely_distributable(
    op1: BinaryOp,
    op2: BinaryOp,
    shift: BinaryOp,
) -> bool {
    (op1 != BinaryOp::Add && op2 != BinaryOp::Add) || shift == BinaryOp::Shl
}

/// `GetInvShift`.
fn inverse(shift: BinaryOp) -> BinaryOp {
    if shift == BinaryOp::LShr { BinaryOp::Shl } else { BinaryOp::LShr }
}

/// The instruction that defines `v` if it is a binary operator `v` is the only
/// user of: its id, operation and operands.
fn one_use_binary(
    c: &Combiner,
    v: Operand,
) -> Option<(InstId, BinaryOp, Operand, Operand)> {
    if !c.has_one_use(v) {
        return None;
    }
    let def = crate::combine_shared::inst_def(c, v)?;
    let instruction = c.function.instruction(def);
    let Opcode::Binary(op) = instruction.opcode else { return None };
    Some((def, op, instruction.operands[0], instruction.operands[1]))
}

/// `CanDistributeBinops`.
fn can_distribute(
    c: &Combiner,
    op1: BinaryOp,
    op2: BinaryOp,
    shift: BinaryOp,
    mask: Operand,
    amount: Operand,
) -> bool {
    // If the BinOp1 is `and` we don't need to check the mask.
    if op1 == BinaryOp::And {
        return true;
    }
    // For all other possible transfers we need complete distributable
    // binop/shift (anything but `add` + `lshr`).
    if !completely_distributable(op1, op2, shift) {
        return false;
    }
    // If BinOp2 is `and`, any mask works.
    if op2 == BinaryOp::And {
        return true;
    }
    // Otherwise, need mask that meets the below requirement.
    // (logic_shift (inv_logic_shift Mask, ShAmt), ShAmt) == Mask
    let Some(back) = fold_constants(c, inverse(shift), mask, amount) else { return false };
    fold_constants(c, shift, back, amount) == Some(mask)
}

/// `MatchBinOp(ShOpnum)`.
fn match_binop(
    c: &mut Combiner,
    inst: InstId,
    shift_operand: usize,
) -> Option<Visit> {
    let Opcode::Binary(top) = c.function.instruction(inst).opcode else { return None };
    let operands = c.function.instruction(inst).operands.clone();
    let [lhs_shift, other] = if shift_operand == 0 { [operands[0], operands[1]] } else { [operands[1], operands[0]] };
    // m_OneUse(m_Shift(m_Value(Y), m_Value(Shift)))
    let (_, sh_y, y, amount) = one_use_binary(c, lhs_shift).filter(|&(_, op, ..)| is_shift(op))?;
    // m_OneUse(m_c_BinOp(m_CombineAnd(m_OneUse(m_Shift(m_Value(X),
    // m_Specific(Shift))), m_Value(ShiftedX)), m_Value(Mask)))
    let (_, bin2, p, q) = one_use_binary(c, other)?;
    let shifted_by_amount = |c: &Combiner, v: Operand| {
        one_use_binary(c, v).filter(|&(_, op, _, a)| is_shift(op) && a == amount).map(|(_, op, x, _)| (op, x))
    };
    let commutative = crate::combine_shared::is_commutative(bin2);
    let (sh_x, x, mask) = shifted_by_amount(c, p)
        .map(|(op, x)| (op, x, q))
        .or_else(|| commutative.then(|| shifted_by_amount(c, q).map(|(op, x)| (op, x, p))).flatten())?;
    // LHS and RHS need same shift opcode
    if sh_y != sh_x {
        return None;
    }
    let shift_op = sh_y;
    // Make sure we have valid binops.
    if !valid_binop(top) || !valid_binop(bin2) {
        return None;
    }
    let q = query(c, Some(inst));
    if shift_op == BinaryOp::AShr {
        let all_ones = pattern::all_ones(&q, mask);
        if matches!(top, BinaryOp::And | BinaryOp::Or | BinaryOp::Xor) && bin2 == BinaryOp::Xor && all_ones {
            let not_x = c.create_not(inst, x);
            let new = c.create_binop(inst, top, y, not_x, Flags::default());
            return Some(c.replace_with_binop(inst, shift_op, new, amount, Flags::default()));
        }
        return None;
    }
    // If BinOp1 == BinOp2 and it's bitwise or shl with add, then just
    // distribute to drop the shift irrelevant of constants.
    if bin2 == top && completely_distributable(top, bin2, shift_op) {
        let inner = c.create_binop(inst, top, x, y, Flags::default());
        let shifted = c.create_binop(inst, shift_op, inner, amount, Flags::default());
        return Some(c.replace_with_binop(inst, top, shifted, mask, Flags::default()));
    }
    // Otherwise we can only distribute by constant shifting the mask, so
    // ensure we have constants.
    if !pattern::imm_constant(&q, amount) || !pattern::imm_constant(&q, mask) {
        return None;
    }
    // Check if we can distribute the binops.
    if !can_distribute(c, top, bin2, shift_op, mask, amount) {
        return None;
    }
    let new_mask = fold_constants(c, inverse(shift_op), mask, amount)?;
    let inner = c.create_binop(inst, bin2, x, new_mask, Flags::default());
    let outer = c.create_binop(inst, top, y, inner, Flags::default());
    Some(c.replace_with_binop(inst, shift_op, outer, amount, Flags::default()))
}

/// `foldBinOpShiftWithShift(I)`.
pub fn fold_binop_shift_with_shift(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    match_binop(c, inst, 0).or_else(|| match_binop(c, inst, 1))
}

#[cfg(test)]
#[path = "combine_shift_binop_tests.rs"]
mod tests;
