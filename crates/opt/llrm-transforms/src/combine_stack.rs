//! InstCombine's `llvm.stackrestore`: the removals of
//! `InstCombinerImpl::visitCallInst`
//! ([InstCombineCalls.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCalls.cpp#L3725)).
//!
//! # How it works
//! A restore is dropped when nothing could have moved the stack since its
//! save: the save is in the same block with no alloca, call with side effects
//! or restore of another save between them (the arrays were removed); when a
//! later restore in the block follows with no alloca or call between; or when
//! the block returns with none between the restore and the return. A save
//! nothing reads is then dead (`wouldInstructionBeTriviallyDead`).
//!
//! # Example
//! ```text
//! %sp = call ptr @llvm.stacksave.p0()      ==>   (nothing)
//! call void @llvm.stackrestore.p0(ptr %sp)
//! ```
//!
//! MIRROR: the three removals and their order.
//!
//! # Used by
//! `combine` (its `visit_call`, the `llvm.stackrestore` intrinsic).

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::pattern::{self, IntrinsicId};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;

/// What the scan of `stackrestore` calls an instruction.
enum Class {
    None,
    Alloca,
    StackRestore,
    CallWithSideEffects,
}

impl Combiner<'_> {
    /// Whether `call` is `llvm.stackrestore`.
    pub(crate) fn is_stack_restore(
        &self,
        call: InstId,
    ) -> bool {
        let q = query(self, Some(call));
        pattern::call_intrinsic(&q, call) == Some(IntrinsicId::StackRestore)
    }

    /// `ClassifyResult` of the pin's `Classify`: an alloca, a restore, a call
    /// that may have side effects (a call that is not an intrinsic always
    /// does; `stacksave` and `read_register` do), or none of them.
    fn classify_for_stack_restore(
        &self,
        inst: InstId,
    ) -> Class {
        let instruction = self.function.instruction(inst);
        match instruction.opcode {
            Opcode::Alloca { .. } => Class::Alloca,
            Opcode::Call(_) => {
                if self.is_stack_restore(inst) {
                    return Class::StackRestore;
                }
                let intrinsic = llrm_mir::memory::callee(self.context, self.function, inst)
                    .and_then(|id| self.env.globals.get(id.0 as usize))
                    .and_then(|callee| callee.name.as_deref())
                    .is_some_and(|name| name.starts_with("llvm."));
                let effects = !intrinsic
                    || crate::local::may_have_side_effects(
                        self.context,
                        self.env.callees,
                        self.env.globals,
                        self.function,
                        inst,
                    );
                if effects { Class::CallWithSideEffects } else { Class::None }
            }
            _ => Class::None,
        }
    }

    /// The `Intrinsic::stackrestore` case of `visitCallInst`.
    pub(crate) fn visit_stack_restore(
        &mut self,
        restore: InstId,
    ) -> Visit {
        let Some(block) = self.function.parent(restore) else { return Visit::None };
        let order: Vec<InstId> = self.function.iter_block(block).collect();
        let at = order.iter().position(|&one| one == restore).expect("the restore is in its block");

        // If the stacksave and the stackrestore are in the same BB, and there
        // is no intervening call, alloca, or stackrestore of a different
        // stacksave, remove the restore. This can happen when variable allocas
        // are DCE'd.
        if let Operand::Value(saved) = self.function.instruction(restore).operands[0]
            && let Some(save) = crate::combine::defining(self.function, Operand::Value(saved))
            && self.is_stack_save(save)
            && let Some(from) = order[..at].iter().position(|&one| one == save)
        {
            let removable = order[from + 1..at]
                .iter()
                .all(
                    |&between| match self.classify_for_stack_restore(between) {
                        Class::None => true,
                        Class::StackRestore => self.function.instruction(between).operands[0] == Operand::Value(saved),
                        Class::Alloca | Class::CallWithSideEffects => false,
                    },
                );
            if removable {
                self.erase(restore);
                return Visit::Erased;
            }
        }

        // Scan down this block to see if there is another stack restore in the
        // same block without an intervening call/alloca.
        let terminator = *order.last().expect("a block ends in a terminator");
        let mut cannot_remove = false;
        for &after in &order[at + 1..order.len() - 1] {
            match self.classify_for_stack_restore(after) {
                Class::None => {}
                // If there is a stackrestore below this one, remove this one.
                Class::StackRestore => {
                    self.erase(restore);
                    return Visit::Erased;
                }
                Class::Alloca | Class::CallWithSideEffects => {
                    cannot_remove = true;
                    break;
                }
            }
        }

        // If the stack restore is in a return, resume, or unwind block and if
        // there are no allocas or calls between the restore and the return,
        // nuke the restore.
        if !cannot_remove && matches!(
            self.function.instruction(terminator).opcode,
            Opcode::Ret | Opcode::Resume
        ) {
            self.erase(restore);
            return Visit::Erased;
        }
        Visit::None
    }

    /// Whether `inst` is a call to `llvm.stacksave`.
    fn is_stack_save(
        &self,
        inst: InstId,
    ) -> bool {
        let q = query(self, Some(inst));
        pattern::call_intrinsic(&q, inst) == Some(IntrinsicId::StackSave)
    }
}

#[cfg(test)]
#[path = "combine_stack_tests.rs"]
mod tests;
