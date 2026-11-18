//! `simplifyAndOrWithOpReplaced`: in `X | Y` try `0` for `Y` inside `X`, in
//! `X & Y` try `-1`, looking only through bitwise operations.
//!
//! Translation of
//! [`simplifyAndOrWithOpReplaced`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L2371)
//! at d1106deb71cc and of its two callers, the tails of `visitAnd` (:2959)
//! and `visitOr` (:4775).
//!
//! MIRROR: depth limit 3, the one-use test that turns the rewrite into a
//! simplification only, and `simplifyBinOp` before a new instruction.
//!
//! DEVIATE 1: the callers run after the other translated folds of the visit
//! (the folds between them and `foldBitwiseLogicWithIntrinsics` are not all
//! translated, and those that are sit in a different order here).
//!
//! # Example
//! `(x ^ 7) | x` reads `x` as 0 inside the xor, which is 7:
//!
//! ```text
//! %a = xor i8 %x, 7
//! %r = or i8 %a, %x      ==>      %r = or i8 %x, 7
//! ```

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::simplify_binop;

/// `Depth >= 3`: how many bitwise operations it looks through.
const MAX_DEPTH: u32 = 3;

/// `isBitwiseLogicOp`.
fn bitwise_logic(op: BinaryOp) -> bool {
    matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)
}

impl Combiner<'_> {
    /// `simplifyAndOrWithOpReplaced(V, Op, RepOp, SimplifyOnly, *this, Depth)`:
    /// what `v` is with `rep` for `op`, or `None`.
    fn simplify_and_or_with_op_replaced(
        &mut self,
        at: InstId,
        v: Operand,
        op: Operand,
        rep: Operand,
        mut simplify_only: bool,
        depth: u32,
    ) -> Option<Operand> {
        if op == rep {
            return None;
        }
        if v == op {
            return Some(rep);
        }
        let inst = self.definition(v)?;
        let Opcode::Binary(opcode) = self.function.instruction(inst).opcode else { return None };
        if !bitwise_logic(opcode) || depth >= MAX_DEPTH {
            return None;
        }
        if !self.has_one_use(v) {
            simplify_only = true;
        }
        let [lhs, rhs] = self.function.instruction(inst).operands[..] else { return None };
        let new_lhs = self.simplify_and_or_with_op_replaced(at, lhs, op, rep, simplify_only, depth + 1);
        let new_rhs = self.simplify_and_or_with_op_replaced(at, rhs, op, rep, simplify_only, depth + 1);
        if new_lhs.is_none() && new_rhs.is_none() {
            return None;
        }
        let (new_lhs, new_rhs) = (new_lhs.unwrap_or(lhs), new_rhs.unwrap_or(rhs));
        if let Some(found) = simplify_binop(self, inst, opcode, new_lhs, new_rhs) {
            return Some(found);
        }
        if simplify_only {
            return None;
        }
        Some(self.create_binop(at, opcode, new_lhs, new_rhs, Flags::default()))
    }
}

/// The tail of `visitAnd` (`and`, `-1`) and of `visitOr` (`or`, `0`): the
/// operation with one operand simplified under the other.
pub(crate) fn fold_and_or_with_op_replaced(
    c: &mut Combiner,
    inst: InstId,
    op: BinaryOp,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let ty = c.type_of(op0);
    let rep = if op == BinaryOp::And { c.all_ones(ty) } else { c.int(ty, 0) };
    if let Some(v) = c.simplify_and_or_with_op_replaced(inst, op0, op1, rep, false, 0) {
        return Some(c.replace_with_binop(inst, op, v, op1, Flags::default()));
    }
    let v = c.simplify_and_or_with_op_replaced(inst, op1, op0, rep, false, 0)?;
    Some(c.replace_with_binop(inst, op, op0, v, Flags::default()))
}

#[cfg(test)]
#[path = "combine_and_or_replaced_tests.rs"]
mod tests;
