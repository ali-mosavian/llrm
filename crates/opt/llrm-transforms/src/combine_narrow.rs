//! `narrowMathIfNoOverflow`: `bo (ext X), (ext Y)` or `bo (ext X), C` as `ext
//! (bo X, Y)` when the math cannot overflow in the narrow width.
//!
//! Translation of
//! [`narrowMathIfNoOverflow`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L2332)
//! at d1106deb71cc.
//!
//! MIRROR: the order of the checks, the swap of a subtraction's operands, and
//! the `nsw` (`sext`) or `nuw` (`zext`) the narrow operation takes.
//!
//! DEVIATE 1: a wide constant is an integer constant (MIR holds no other
//! constant that `getLosslessTrunc` could narrow).
//!
//! # Used by
//! `visitAdd`, `visitSub` (`TryToNarrowDeduceFlags`) and `visitMul`.
//!
//! # Example
//! ```text
//! %a = zext i8 %x to i32                      %n = add nuw i8 %x, 3
//! %r = add i32 %a, 3                   ==>    %r = zext i8 %n to i32
//! ```
//! when `%x + 3` cannot wrap in 8 bits.

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::valuetracking::range::{
    OverflowResult, compute_overflow_for_signed_add, compute_overflow_for_signed_mul, compute_overflow_for_signed_sub,
    compute_overflow_for_unsigned_add, compute_overflow_for_unsigned_mul, compute_overflow_for_unsigned_sub,
};
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_minmax::lossless_trunc;
use crate::combine_shared::{has_one_use, parts, query};

impl Combiner<'_> {
    /// `willNotOverflow(Opcode, X, Y, BO, IsSigned)`.
    fn will_not_overflow(
        &self,
        op: BinaryOp,
        x: Operand,
        y: Operand,
        cxt: InstId,
        signed: bool,
    ) -> bool {
        let q = query(self, Some(cxt));
        let result = match (op, signed) {
            (BinaryOp::Add, true) => compute_overflow_for_signed_add(&q, x, y),
            (BinaryOp::Add, false) => compute_overflow_for_unsigned_add(&q, x, y),
            (BinaryOp::Sub, true) => compute_overflow_for_signed_sub(&q, x, y),
            (BinaryOp::Sub, false) => compute_overflow_for_unsigned_sub(&q, x, y),
            (BinaryOp::Mul, true) => compute_overflow_for_signed_mul(&q, x, y),
            (BinaryOp::Mul, false) => compute_overflow_for_unsigned_mul(&q, x, y, false),
            _ => return false,
        };
        result == OverflowResult::NeverOverflows
    }

    /// `narrowMathIfNoOverflow(BO)`.
    pub(crate) fn narrow_math_if_no_overflow(
        &mut self,
        inst: InstId,
    ) -> Option<Visit> {
        let Opcode::Binary(op) = self.function.instruction(inst).opcode else { return None };
        // We need at least one extended operand.
        let (mut op0, mut op1, _) = parts(self, inst);
        // If this is a sub, we swap the operands since we always want an
        // extension on the RHS. The LHS can be an extension or a constant.
        if op == BinaryOp::Sub {
            std::mem::swap(&mut op0, &mut op1);
        }
        let (ext, mut x) = {
            let q = query(self, Some(inst));
            match pattern::cast(&q, op0, CastOp::SExt) {
                Some((_, _, x)) => (CastOp::SExt, x),
                None => (CastOp::ZExt, pattern::cast(&q, op0, CastOp::ZExt)?.2),
            }
        };
        let signed = ext == CastOp::SExt;

        // If both operands are the same extension from the same source type
        // and we can eliminate at least one (hasOneUse), this might work.
        let same = {
            let q = query(self, Some(inst));
            pattern::cast(&q, op1, ext).map(|(_, _, y)| y).filter(|&y| {
                self.type_of(x) == self.type_of(y) && (has_one_use(self, op0) || has_one_use(self, op1))
            })
        };
        let mut y = match same {
            Some(y) => y,
            None => {
                // If that did not match, see if we have a suitable constant
                // operand. Truncating and extending must produce the same
                // constant.
                if !has_one_use(self, op0) {
                    return None;
                }
                let (wide, bits) = query(self, Some(inst)).int_constant(op1)?;
                let small = self.context.types.int_bits(self.type_of(x))?;
                let narrow = lossless_trunc(wide, bits, small, ext)?;
                self.int(self.type_of(x), narrow)
            }
        };

        // Swap back now that we found our operands.
        if op == BinaryOp::Sub {
            std::mem::swap(&mut x, &mut y);
        }

        // Both operands have narrow versions. Last step: the math must not
        // overflow in the narrow width.
        if !self.will_not_overflow(op, x, y, inst, signed) {
            return None;
        }

        // bo (ext X), (ext Y) --> ext (bo X, Y)
        // bo (ext X), C       --> ext (bo X, C')
        let wrap = if signed { Flags::NSW } else { Flags::NUW };
        let narrow = self.create_binop(inst, op, x, y, wrap);
        let ty = self.function.instruction(inst).ty;
        Some(self.replace_with_new(inst, Opcode::Cast(ext), ty, vec![narrow], Flags::default()))
    }
}
