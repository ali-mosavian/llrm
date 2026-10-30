//! Reassociate's negative floating-point constants: a negated constant out of
//! a product, so that `fadd` becomes `fsub` and constants fold and CSE.
//!
//! Translation of `getNegatibleInsts`, `canonicalizeNegFPConstantsForOp` and
//! `canonicalizeNegFPConstants`
//! ([Reassociate.cpp:2165-2288](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L2165))
//! at d1106deb71cc.
//!
//! MIRROR: the one-use `fmul`/`fdiv` expression under an `fadd`/`fsub` is
//! searched for negative constants; each is made positive, and an odd number
//! turns the `fadd` into an `fsub` (or back) so that the sign is not lost.
//!
//! DEVIATE: `getNegatibleInsts` keeps its own stack, not recursion: a chain of
//! one-use products is as deep as the function is long.
//!
//! # Used by
//! `crate::reassociate_inst` (`OptimizeInst`).
//!
//! # Example
//! `x + (y * -2.0)` is `x - (y * 2.0)`.

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};

use crate::reassociate::Reassociator;
use crate::reassociate_fp::{float_constant, float_value};

impl Reassociator<'_> {
    /// `m_APFloat(C)` with `C->isNegative()`.
    fn is_negative_float(
        &self,
        v: Operand,
    ) -> bool {
        float_value(self.context, v).is_some_and(|x| x.is_sign_negative())
    }

    /// `getNegatibleInsts`: the instructions of the one-use `fmul`/`fdiv`
    /// expression `v` with a negative constant operand.
    fn negatible_insts(
        &self,
        v: Operand,
    ) -> Vec<InstId> {
        let mut candidates = Vec::new();
        let mut stack = vec![v];
        while let Some(v) = stack.pop() {
            // Only one-use instructions: combining negations does not justify
            // replicating instructions.
            let Some(inst) = self.inst_of(v).filter(|_| self.has_one_use(v)) else { continue };
            let (op, a, b) = match self.function.instruction(inst).opcode {
                Opcode::Binary(op @ (BinaryOp::FMul | BinaryOp::FDiv)) => {
                    (op, self.operand(inst, 0), self.operand(inst, 1))
                }
                _ => continue,
            };
            if op == BinaryOp::FMul {
                // Not expecting non-canonical code here: wait.
                if self.is_constant(a) {
                    continue;
                }
                if self.is_negative_float(b) {
                    candidates.push(inst);
                }
            } else {
                if self.is_constant(a) && self.is_constant(b) {
                    continue;
                }
                if self.is_negative_float(a) || self.is_negative_float(b) {
                    candidates.push(inst);
                }
            }
            stack.push(b);
            stack.push(a);
        }
        candidates
    }

    /// `canonicalizeNegFPConstantsForOp`: `i` is an `fadd`/`fsub` of `op` and
    /// `other`.
    fn canonicalize_neg_fp_constants_for_op(
        &mut self,
        i: InstId,
        op: InstId,
        other: Operand,
    ) -> Option<InstId> {
        let op_value = self.result(op);
        let candidates = self.negatible_insts(op_value);
        if candidates.is_empty() {
            return None;
        }
        // Not `x + (-C * y)` to `x - (C * y)` if the subtract would be broken
        // up again: an endless loop.
        let is_fsub = self.binop(i) == Some(BinaryOp::FSub);
        let needs_subtract = !is_fsub && candidates.len() % 2 == 1;
        if needs_subtract && self.should_break_up_subtract(i) {
            return None;
        }
        for negatible in candidates.iter().copied() {
            let ty = self.function.instruction(negatible).ty;
            for index in 0..2 {
                let operand = self.operand(negatible, index);
                if let Some(x) = float_value(self.context, operand) {
                    let positive = float_constant(self.context, ty, x.abs());
                    self.function.set_operand(negatible, index, positive);
                    self.changed = true;
                }
            }
        }
        // The negations cancelled.
        if candidates.len() % 2 == 0 {
            return Some(i);
        }
        // Negate the last operand by flipping the opcode.
        let ty = self.function.instruction(i).ty;
        let fast = self.flags(i).intersect(Flags::FAST);
        let flipped = if is_fsub { BinaryOp::FAdd } else { BinaryOp::FSub };
        let new = self.insert_before(i, Opcode::Binary(flipped), ty, vec![other, op_value], fast);
        let value = self.result(new);
        self.replace_all_uses(i, value);
        self.redo.insert(i);
        Some(new)
    }

    /// `canonicalizeNegFPConstants`:
    /// `Other + (subtree)`, `(subtree) + Other` and `Other - (subtree)`.
    pub fn canonicalize_neg_fp_constants(
        &mut self,
        mut i: InstId,
    ) -> InstId {
        let one_use_inst = |this: &Self, v: Operand| this.inst_of(v).filter(|_| this.has_one_use(v));
        let (fadd, fsub) = (BinaryOp::FAdd, BinaryOp::FSub);
        if self.binop(i) == Some(fadd)
            && let Some(op) = one_use_inst(self, self.operand(i, 1))
            && let Some(r) = self.canonicalize_neg_fp_constants_for_op(i, op, self.operand(i, 0))
        {
            i = r;
        }
        if self.binop(i) == Some(fadd)
            && let Some(op) = one_use_inst(self, self.operand(i, 0))
            && let Some(r) = self.canonicalize_neg_fp_constants_for_op(i, op, self.operand(i, 1))
        {
            i = r;
        }
        if self.binop(i) == Some(fsub)
            && let Some(op) = one_use_inst(self, self.operand(i, 1))
            && let Some(r) = self.canonicalize_neg_fp_constants_for_op(i, op, self.operand(i, 0))
        {
            i = r;
        }
        i
    }
}
