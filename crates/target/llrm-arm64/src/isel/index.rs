//! An index that is a constant shift or a 32-bit extension of a value, read
//! only as the one variable index of a `getelementptr`, is part of the
//! addressing mode (`ldr w0, [x0, w1, sxtw #2]`, `strh w2, [x0, x1, lsl #1]`;
//! `AArch64DAGToDAGISel::SelectAddrModeWRO`/`SelectAddrModeXRO`).
//!
//! The shift or extension is skipped where it is and the `getelementptr`
//! builds the indexed address from the value it read. Which instructions fold
//! is decided from the IR alone ([`Selector::folds_into_index`]), so the
//! instruction and its reader agree.

use llrm_mir::{BinaryOp, CastOp, Instruction, Opcode, Operand};

use super::Selected;
use super::Selector;
use super::val::{AddrExpr, Index};
use crate::mc::inst::Extend;

/// How the folded instruction contributes to the address.
enum Fold {
    /// `index << amount`.
    Shift(u8),
    Extend(Extend),
}

impl Selector<'_> {
    /// The scale of the one variable index of `gep`, if it has exactly one and
    /// the address built from it can carry an index: a base that is not
    /// itself an indexed address, a power-of-two scale up to 16.
    fn single_index_scale(
        &self,
        gep: &Instruction,
        source: llrm_mir::TypeId,
    ) -> Option<u64> {
        let variable: Vec<_> = gep
            .operands
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, operand)| matches!(operand, Operand::Value(_)))
            .collect();
        let [_] = variable[..] else { return None };
        if let Operand::Value(base) = gep.operands[0]
            && super::value_def(self.function, base)
                .is_some_and(|def| matches!(
                    self.function.instruction(def).opcode,
                    Opcode::GetElementPtr { .. }
                ))
        {
            return None;
        }
        let indices: Vec<Option<i128>> = gep.operands[1..]
            .iter()
            .map(|operand| match operand {
                Operand::Constant(id) => match self.module.context.get(*id).kind {
                    llrm_mir::ConstantKind::Int(value) => Some(value as i128),
                    llrm_mir::ConstantKind::Zero => Some(0),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        let (_, scales) = self.layout.collect_offset(self.types(), source, &indices);
        let [(_, scale)] = scales[..] else { return None };
        (scale.is_power_of_two() && scale <= 16).then_some(scale)
    }

    fn fold_of(
        &self,
        instruction: &Instruction,
    ) -> Option<Fold> {
        let from = self.operand_type(instruction.operands[0]).ok()?;
        match instruction.opcode {
            Opcode::Cast(op @ (CastOp::SExt | CastOp::ZExt))
                if self.types().int_bits(from) == Some(32) && self.types().int_bits(instruction.ty) == Some(64) =>
            {
                Some(Fold::Extend(if op == CastOp::SExt { Extend::Sxtw } else { Extend::Uxtw }))
            }
            Opcode::Binary(BinaryOp::Shl) if self.types().int_bits(instruction.ty) == Some(64) => {
                let Operand::Constant(amount) = instruction.operands[1] else { return None };
                match self.module.context.get(amount).kind {
                    llrm_mir::ConstantKind::Int(amount) if amount < 4 => Some(Fold::Shift(amount as u8)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Whether `instruction` (a shift or extension) is read only as the
    /// variable index of a `getelementptr` of its block that builds an
    /// indexed address from it.
    pub(super) fn folds_into_index(
        &self,
        instruction: &Instruction,
    ) -> bool {
        let Some(result) = instruction.result else { return false };
        if self.use_count(result) != 1 || self.fold_of(instruction).is_none() {
            return false;
        }
        let Some(user) = self.function.users(result).into_iter().next() else { return false };
        let gep = self.function.instruction(user.user);
        let Opcode::GetElementPtr { source } = gep.opcode else { return false };
        let own = super::value_def(self.function, result).and_then(|inst| self.function.parent(inst));
        own == self.function.parent(user.user)
            && user.index != 0
            && self.single_index_scale(gep, source).is_some_and(|scale| match self.fold_of(instruction) {
                Some(Fold::Shift(amount)) => (scale << amount) <= 16,
                _ => true,
            })
    }

    /// Whether `operand` is an instruction left to the `getelementptr` that
    /// reads it.
    pub(super) fn is_folded_index(
        &self,
        operand: Operand,
    ) -> bool {
        let Operand::Value(value) = operand else { return false };
        super::value_def(self.function, value).is_some_and(|def| self.indexed.contains(&def))
    }

    /// The index `operand` makes in `address` if it is a folded instruction:
    /// its source register, extended or shifted.
    pub(super) fn folded_index(
        &mut self,
        operand: Operand,
        scale: u64,
    ) -> Selected<Option<Index>> {
        let Operand::Value(value) = operand else { return Ok(None) };
        let Some(def) = super::value_def(self.function, value).filter(|def| self.indexed.contains(def)) else {
            return Ok(None);
        };
        let folded = self.function.instruction(def).clone();
        let Some(fold) = self.fold_of(&folded) else { return Ok(None) };
        let g = self.operand_g(folded.operands[0])?;
        let base = scale.trailing_zeros() as u8;
        Ok(Some(match fold {
            Fold::Shift(amount) => Index { reg: g.reg, shift: base + amount, extend: Extend::Uxtx },
            Fold::Extend(extend) => Index { reg: g.reg, shift: base, extend },
        }))
    }
}

#[allow(dead_code)]
fn unused(_: AddrExpr) {}
