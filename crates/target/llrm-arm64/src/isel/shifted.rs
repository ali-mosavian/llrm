//! A constant shift that only an `add`, `sub`, `and`, `or` or `xor` of its own
//! block reads is that instruction's shifted operand (`add x0, x1, x2, lsl #3`;
//! `AArch64DAGToDAGISel::SelectShiftedRegister`).
//!
//! The shift is skipped where it is, and the reader emits the one instruction.
//! Which operand of the reader the shift takes is decided by the IR alone
//! ([`Selector::shifted_slot`]), so the shift and its reader agree.

use llrm_mir::{BinaryOp, Instruction, Opcode, Operand, ValueId};

use super::val::{G, Known, width_of};
use super::{Selected, Selector};
use crate::mc::inst::*;

fn is_shift(op: BinaryOp) -> bool {
    matches!(op, BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr)
}

fn commutes(op: BinaryOp) -> bool {
    !matches!(op, BinaryOp::Sub)
}

impl Selector<'_> {
    /// The constant shift `value` is, if it could be a shifted operand: 32 or
    /// 64 bits, read once.
    fn shift_shaped(
        &self,
        operand: Operand,
    ) -> Option<(BinaryOp, Shift, u32)> {
        let Operand::Value(value) = operand else { return None };
        let def = super::value_def(self.function, value)?;
        let shift = self.function.instruction(def);
        let Opcode::Binary(op) = shift.opcode else { return None };
        let bits = self.types().int_bits(shift.ty)?;
        let Operand::Constant(amount) = shift.operands[1] else { return None };
        let amount = match self.module.context.get(amount).kind {
            llrm_mir::ConstantKind::Int(amount) => u32::try_from(amount).ok()?,
            _ => return None,
        };
        (is_shift(op) && matches!(bits, 32 | 64) && amount < bits && self.use_count(value) == 1).then_some((
            op,
            match op {
                BinaryOp::Shl => Shift::Lsl,
                BinaryOp::LShr => Shift::Lsr,
                _ => Shift::Asr,
            },
            amount,
        ))
    }

    /// The operand of `reader` that a shift can be folded into: the second,
    /// else (where the operation commutes) the first; the other operand is
    /// a register value an immediate form or a multiply-add does not take.
    fn shifted_slot(
        &self,
        reader: &Instruction,
    ) -> Option<usize> {
        let Opcode::Binary(op @ (BinaryOp::Add | BinaryOp::Sub | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)) =
            reader.opcode
        else {
            return None;
        };
        if !matches!(self.types().int_bits(reader.ty), Some(32 | 64)) {
            return None;
        }
        let [first, second] = reader.operands[..] else { return None };
        let slot = if self.shift_shaped(second).is_some() {
            1
        } else if commutes(op) && self.shift_shaped(first).is_some() {
            0
        } else {
            return None;
        };
        let other = reader.operands[1 - slot];
        let takes_other = |operand: Operand| match operand {
            Operand::Constant(_) => true,
            Operand::Value(value) => self.accumulated(value),
            _ => false,
        };
        (!takes_other(other)).then_some(slot)
    }

    /// Whether the shift `instruction` is read by an instruction it folds into.
    pub(super) fn folds_into_reader(
        &self,
        instruction: &Instruction,
    ) -> bool {
        let Some(result) = instruction.result else { return false };
        if self.shift_shaped(Operand::Value(result)).is_none() {
            return false;
        }
        let Some(user) = self.function.users(result).into_iter().next() else { return false };
        let reader = self.function.instruction(user.user);
        let own = super::value_def(self.function, result).and_then(|inst| self.function.parent(inst));
        // An `and` a comparison tests is selected there, as a `tst`.
        own == self.function.parent(user.user)
            && !reader.result.is_some_and(|reader| self.tested_by_compare(reader))
            && self.shifted_slot(reader).is_some_and(|slot| reader.operands[slot] == Operand::Value(result))
    }

    /// `reader` as one instruction over the shift it folds, if it has one.
    pub(super) fn shifted_operand(
        &mut self,
        op: BinaryOp,
        bits: u32,
        reader: &Instruction,
    ) -> Selected<Option<G>> {
        let Some(slot) = self.shifted_slot(reader) else { return Ok(None) };
        let Operand::Value(shifted) = reader.operands[slot] else { return Ok(None) };
        let Some(def) = super::value_def(self.function, shifted).filter(|def| self.fused.contains(def)) else {
            return Ok(None);
        };
        let shift = self.function.instruction(def).clone();
        let Some((_, kind, amount)) = self.shift_shaped(Operand::Value(shifted)) else { return Ok(None) };
        let (other, source) = (self.src(reader.operands[1 - slot])?, self.src(shift.operands[0])?);
        let (a, b) = (self.gpr_of(other)?, self.gpr_of(source)?);
        let width = width_of(bits);
        let rd = self.mf.gpr();
        let amount = amount as u8;
        self.inst(match op {
            BinaryOp::Add | BinaryOp::Sub => Inst::AddSubShifted {
                op: AddSub { subtract: op == BinaryOp::Sub, set_flags: false },
                width,
                rd,
                rn: a.reg,
                rm: b.reg,
                shift: kind,
                amount,
            },
            _ => Inst::LogicShifted {
                op: match op {
                    BinaryOp::And => Logic::And,
                    BinaryOp::Or => Logic::Orr,
                    _ => Logic::Eor,
                },
                invert: false,
                width,
                rd,
                rn: a.reg,
                rm: b.reg,
                shift: kind,
                amount,
            },
        });
        Ok(Some(G { reg: rd, bits, known: if bits == 32 { Known::Zero } else { Known::Nothing } }))
    }
}

#[allow(dead_code)]
fn unused(_: ValueId) {}
