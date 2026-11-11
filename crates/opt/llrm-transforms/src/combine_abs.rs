//! InstCombine's `llvm.abs`: the `Intrinsic::abs` case of
//! `InstCombinerImpl::visitCallInst`
//! ([InstCombineCalls.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCalls.cpp#L2177)).
//!
//! # How it works
//! The operand's sign never matters to `abs`, so a negation, a select between
//! a value and its negation, and a multiply by another `abs` are looked
//! through; a sign extension narrows the `abs`; `abs (srem x, 2)` is `x & 1`.
//!
//! # Example
//! ```text
//! call i32 @llvm.abs.i32(i32 %n, i1 false)   ==>   call i32 @llvm.abs.i32(i32 %x, i1 false)
//! ; %n = sub i32 0, %x
//! ```
//!
//! MIRROR: the order of the folds and each pattern.
//!
//! DEVIATE 1: `getKnownSignOrZero` (`abs(x) -> x` where x >= 0, `-x` where
//! x < 0) is not translated; it needs its ValueTracking function first. Exit
//! 2026-11-09.
//!
//! DEVIATE 2: no vectors.
//!
//! # Used by
//! `combine` (its `visit_call`, the `llvm.abs` intrinsic).

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;

impl Combiner<'_> {
    /// The `llvm.abs` case of `visitCallInst`.
    pub(crate) fn visit_abs(
        &mut self,
        call: InstId,
    ) -> Visit {
        let [operand, int_min_flag, ..] = self.function.instruction(call).operands[..] else { return Visit::None };
        let ty = self.function.instruction(call).ty;
        let i1 = self.context.types.int(1);
        let q = query(self, Some(call));
        let int_min_is_poison = pattern::one(&q, int_min_flag);
        let name = format!("llvm.abs.i{}", q.scalar_bits(operand));
        // abs(-x) -> abs(x)
        if let Some((_, x, flags)) = pattern::neg(&q, operand) {
            let poison = int_min_is_poison || flags.contains(Flags::NSW);
            let flag = self.int(i1, u128::from(poison));
            return Visit::Replace(self.create_intrinsic_named(call, &name, ty, &[x, flag]));
        }
        // abs(select(c, -x, x)) -> abs(x), either arm order
        if let Some((_, _, _, a, b)) = pattern::select(&q, operand) {
            for (negated, plain) in [(a, b), (b, a)] {
                if pattern::neg(&q, negated).is_some_and(|(_, x, _)| x == plain) {
                    return Visit::Replace(self.create_intrinsic_named(call, &name, ty, &[plain, int_min_flag]));
                }
            }
        }
        // abs(a * abs(b)) -> abs(a * b)
        if let Some((_, instruction, lhs, rhs)) = pattern::binary(&q, operand, BinaryOp::Mul)
            && self.has_one_use(operand)
        {
            let nsw = instruction.flags.contains(Flags::NSW) && int_min_is_poison;
            for (x, other) in [(lhs, rhs), (rhs, lhs)] {
                if let Some((_, args)) = pattern::intrinsic_call(&q, other, "llvm.abs") {
                    let flags = if nsw { Flags::NSW } else { Flags::default() };
                    let product = self.create_binop(call, BinaryOp::Mul, x, args[0], flags);
                    return Visit::Replace(self.create_intrinsic_named(call, &name, ty, &[product, int_min_flag]));
                }
            }
        }
        // abs (sext X) --> zext (abs X*), the narrow abs without the nsw bit
        if let Some((_, instruction)) = q.instruction_of(operand)
            && instruction.opcode == Opcode::Cast(CastOp::SExt)
            && self.has_one_use(operand)
        {
            let x = instruction.operands[0];
            let narrow = format!("llvm.abs.i{}", q.scalar_bits(x));
            let narrow_ty = self.type_of(x);
            let flag = self.int(i1, 0);
            let abs = self.create_intrinsic_named(call, &narrow, narrow_ty, &[x, flag]);
            return Visit::Replace(self.create_cast(call, CastOp::ZExt, abs, ty, Flags::default()));
        }
        // abs (srem X, 2) --> and X, 1
        if let Some((_, _, x, two)) = pattern::binary(&q, operand, BinaryOp::SRem)
            && pattern::constant_int_u64(&q, two) == Some(2)
        {
            let one = self.int(ty, 1);
            return self.replace_with_binop(call, BinaryOp::And, x, one, Flags::default());
        }
        Visit::None
    }
}

#[cfg(test)]
#[path = "combine_abs_tests.rs"]
mod tests;
