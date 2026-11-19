//! InstCombine's call that returns one of its arguments: the call's value is
//! that argument. Translated from the `getReturnedArgOperand` step of
//! `InstCombinerImpl::visitCallBase`
//! ([InstCombineCalls.cpp#L5166](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineCalls.cpp#L5166)).
//!
//! # How it works
//! A parameter marked `returned` (declared, or inferred by
//! `addArgumentReturnedAttrs`) comes back as the call's value, so every use of
//! the call reads the argument instead. The call stays: it may do more than
//! return.
//!
//! # Example
//! ```text
//! %r = call i32 @id(i32 returned %x)      ; use(%r)
//!   ==>  use(%x)
//! ```
//!
//! MIRROR: not a `musttail` call, the call has a use, the argument replaces
//! it.
//!
//! DEVIATE 1: the argument must have the call's type; the pin also casts
//! where `canLosslesslyBitCastTo` holds. Reason: the verifier gives a
//! `returned` parameter its function's return type, so only a call through a
//! mismatched prototype differs, and `transformConstExprCastCall` takes those
//! first. Exit: none.
//!
//! # Used by
//! `combine_memintrinsics` (`visit_call`).

use llrm_mir::module::InstId;
use llrm_mir::opcode::{Opcode, Tail};
use llrm_mir::valuetracking::pattern::returned_argument_operand;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;

impl Combiner<'_> {
    /// The `getReturnedArgOperand` step of `visitCallBase`.
    pub(crate) fn replace_call_with_returned_argument(
        &mut self,
        call: InstId,
    ) -> Visit {
        let instruction = self.function.instruction(call);
        let Opcode::Call(info) = &instruction.opcode else { return Visit::None };
        let Some(result) = instruction.result else { return Visit::None };
        if info.tail == Tail::MustTail || self.function.users(result).is_empty() {
            return Visit::None;
        }
        let returned = {
            let q = query(self, Some(call));
            returned_argument_operand(&q, self.function.instruction(call))
        };
        let Some(returned) = returned else { return Visit::None };
        if self.function.operand_type(self.context, returned) != Some(self.function.instruction(call).ty) {
            return Visit::None;
        }
        self.replace_inst_with(call, returned);
        Visit::Changed
    }
}

#[cfg(test)]
#[path = "combine_returned_arg_tests.rs"]
mod tests;
