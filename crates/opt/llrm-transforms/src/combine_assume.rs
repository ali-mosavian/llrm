//! InstCombine's `llvm.assume`: the condition folds of
//! `InstCombinerImpl::visitCallInst`
//! ([InstCombineCalls.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCalls.cpp#L3837)).
//!
//! # How it works
//! `assume(a && b)` is `assume(a); assume(b)`, and `assume(!(a || b))` is
//! `assume(!a); assume(!b)`: each condition then reaches ValueTracking alone.
//! An assume of a constant true is erased; of false, poison or undef it is
//! unreachable code, which becomes a store of `true` to a poison pointer
//! (`CreateNonTerminatorUnreachable`) before the assume is erased.
//!
//! # Example
//! ```text
//! call void @llvm.assume(i1 %ab)      ==>   call void @llvm.assume(i1 %a)
//! ; %ab = and i1 %a, %b                     call void @llvm.assume(i1 %b)
//! ```
//!
//! MIRROR: the order of the folds and each pattern.
//!
//! DEVIATE 1: MIR has no operand bundles (#2203), so the bundle loop, the
//! `nonnull` and `align` bundles made from an `icmp ne ptr, null` or an
//! alignment mask, and the `hasOperandBundles` early exit are not translated.
//! DEVIATE 2: no `AssumptionCache::updateAffectedValues`; the cache is built
//! from the function when asked.
//!
//! # Used by
//! `combine` (its `visit_call`, the `llvm.assume` intrinsic).

use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;

impl Combiner<'_> {
    /// The `llvm.assume` case of `visitCallInst`.
    pub(crate) fn visit_assume(
        &mut self,
        call: InstId,
    ) -> Visit {
        let condition = self.function.instruction(call).operands[0];
        let (and, or_not) = {
            let q = query(self, Some(call));
            (
                pattern::logical_and(&q, condition),
                pattern::not(&q, condition).and_then(|inner| pattern::logical_or(&q, inner)),
            )
        };
        // Canonicalize assume(a && b) -> assume(a); assume(b);
        if let Some((a, b)) = and {
            self.create_assumption(call, a);
            self.create_assumption(call, b);
            self.erase(call);
            return Visit::Erased;
        }
        // assume(!(a || b)) -> assume(!a); assume(!b);
        if let Some((a, b)) = or_not {
            let not_a = self.create_not(call, a);
            self.create_assumption(call, not_a);
            let not_b = self.create_not(call, b);
            self.create_assumption(call, not_b);
            self.erase(call);
            return Visit::Erased;
        }
        // Remove assumes on true/false.
        let constant = match condition {
            Operand::Constant(id) => Some(self.context.get(id).kind.clone()),
            _ => None,
        };
        let (true_condition, unreachable) = match constant {
            Some(ConstantKind::Int(bits)) => (bits != 0, bits == 0),
            Some(ConstantKind::Undef | ConstantKind::Poison) => (false, true),
            _ => return Visit::None,
        };
        if unreachable && !true_condition {
            self.create_non_terminator_unreachable(call);
        }
        self.erase(call);
        Visit::Erased
    }

    /// `Builder.CreateAssumption(cond)` before `at`.
    fn create_assumption(
        &mut self,
        at: InstId,
        condition: Operand,
    ) {
        let void = self.context.types.void();
        self.insert_intrinsic_call(at, "llvm.assume", void, &[condition]);
    }

    /// `CreateNonTerminatorUnreachable(InsertAt)`: `store i1 true, ptr poison`.
    fn create_non_terminator_unreachable(
        &mut self,
        at: InstId,
    ) {
        let i1 = self.context.types.int(1);
        let pointer = self.context.types.ptr(0);
        let truth = self.int(i1, 1);
        let poison = Operand::Constant(self.context.constant(Constant { ty: pointer, kind: ConstantKind::Poison }));
        let void = self.context.types.void();
        self.insert_before(
            at,
            Opcode::Store { align: None, volatile: false },
            void,
            vec![truth, poison],
            Flags::default(),
        );
    }
}

#[cfg(test)]
#[path = "combine_assume_tests.rs"]
mod tests;
