//! The product of two 32-bit values that is extended to 64 bits is one widening multiply:
//! `sext (mul nsw a, b)` is `smull`, and added to a 64-bit value `smaddl` (`zext (mul nuw ...)` the
//! unsigned `umull`/`umaddl`). With no overflow the 64-bit product of the extended factors is the
//! extension of the 32-bit one, and the instructions read the factors' low halves as they are.
//!
//! The multiply is skipped where it is. The extension either makes the `smull` itself or, when an
//! `add` or `sub` of its block reads it (`accumulated`), is skipped too, and that reader emits the
//! `smaddl`/`smsubl`. Both are decided from the IR alone, so each side agrees.

use llrm_mir::{BinaryOp, CastOp, Flags, Instruction, Opcode};

use super::val::{G, Known, Val};
use super::{Selected, Selector};
use crate::mc::inst::*;

impl Selector<'_> {
    /// The 32-bit `mul` that `extension`, a `sext` or `zext` to 64 bits, extends, when it is
    /// `nsw` (`nuw`), read only by `extension`, in its block, with no constant factor.
    pub(super) fn widened_product(
        &self,
        extension: &Instruction,
    ) -> Option<Instruction> {
        let flag = match extension.opcode {
            Opcode::Cast(CastOp::SExt) => Flags::NSW,
            Opcode::Cast(CastOp::ZExt) => Flags::NUW,
            _ => return None,
        };
        let llrm_mir::Operand::Value(value) = *extension.operands.first()? else { return None };
        // An index the addressing mode extends is the 32-bit product itself.
        if self.types().int_bits(extension.ty) != Some(64) || self.use_count(value) != 1 || self.folds_into_index(extension) {
            return None;
        }
        let def = super::value_def(self.function, value)?;
        let product = self.function.instruction(def);
        let same_block = self.function.parent(def) == extension.result.and_then(|result| {
            super::value_def(self.function, result).and_then(|inst| self.function.parent(inst))
        });
        (product.opcode == Opcode::Binary(BinaryOp::Mul)
            && product.flags.contains(flag)
            && self.types().int_bits(product.ty) == Some(32)
            && same_block
            && product.operands.iter().all(|one| matches!(one, llrm_mir::Operand::Value(_))))
        .then(|| product.clone())
    }

    /// Whether the product `value` is read only by an extension that makes a widening multiply.
    pub(super) fn is_widened_product(
        &self,
        value: llrm_mir::ValueId,
    ) -> bool {
        if self.use_count(value) != 1 {
            return false;
        }
        let Some(user) = self.function.users(value).into_iter().next() else { return false };
        self.widened_product(self.function.instruction(user.user)).is_some()
    }

    /// `smull`/`umull` of the factors of `product`, for the extension `instruction`.
    pub(super) fn widening_multiply(
        &mut self,
        instruction: &Instruction,
        product: &Instruction,
    ) -> Selected<()> {
        let signed = instruction.opcode == Opcode::Cast(CastOp::SExt);
        let (lhs, rhs) = (self.src(product.operands[0])?, self.src(product.operands[1])?);
        let (a, b) = (self.gpr_of(lhs)?, self.gpr_of(rhs)?);
        let rd = self.mf.gpr();
        let op = if signed { Mul3::Smaddl } else { Mul3::Umaddl };
        self.inst(Inst::ThreeSource { op, width: Width::X, rd, rn: a.reg, rm: b.reg, ra: Gpr::R31 });
        self.define(instruction.result, super::Src::Val(Val::Int(G { reg: rd, bits: 64, known: Known::Nothing })))
    }
}
