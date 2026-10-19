//! InstCombine's stores and allocas: the store that cannot matter is erased,
//! the one that cannot execute stores poison; an alloca's count is an array
//! type and a zero-size alloca is one at the top of the entry block. Translated
//! from `visitAllocaInst`
//! ([:509](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineLoadStoreAlloca.cpp#L509)),
//! `simplifyAllocaArraySize` (:182) and `visitStoreInst`
//! ([InstCombineLoadStoreAlloca.cpp:1473](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineLoadStoreAlloca.cpp#L1473))
//! with `equivalentAddressValues` (:1452) and `canSimplifyNullStoreOrGEP`
//! (:1022).
//!
//! # How it works
//! A volatile store is left alone. Otherwise the store is erased when it writes
//! an alloca nothing else reads (directly or through a one-use GEP), when a
//! store of the same type to the same address stands within the 6 instructions
//! before it with nothing that touches memory between (that earlier store is
//! erased instead), when it writes back the value just loaded from that
//! address, or when it stores `undef`. A store to null (or a GEP of null) in an
//! address space where null is not defined stores poison.
//!
//! ```text
//! store i16 1, ptr %p      ; erased
//! %x = add i16 %a, 1
//! store i16 %x, ptr %p
//! ```
//!
//! MIRROR: every check and its order, the scan of 6 and what ends it.
//!
//! DEVIATE 1: `combineStoreToValueType`, `unpackStoreToAggregate` and
//! `replaceGEPIdxWithZero` are the next slices of this file, not yet
//! translated.
//!
//! DEVIATE 2: a store to constant memory (`!isModSet(AA->getModRefInfoMask)`)
//! is not erased: the combine holds no alias analysis.
//!
//! DEVIATE 3: a store to an `undef` pointer is not a marker that removes what
//! follows (`handleUnreachableFrom` is a CFG edit this pass does not make, see
//! the module doc of `combine`); the store stays.
//!
//! DEVIATE 4: `simplifyNonNullOperand` on the pointer and the byte and
//! protected-field rewrites are not made (MIR has no byte type or
//! `llvm.protected.field.ptr`; the non-null rewrite is a later slice).
//!
//! DEVIATE 5: what ends the scan of 6 is a load, a store, a call, an invoke or
//! a resume: MIR has no other instruction that reads memory, writes it or
//! throws.
//!
//! DEVIATE 6: an alloca only copied from constant memory
//! (`isOnlyCopiedFromConstantMemory`, `PointerReplacer`) and `visitAllocSite`
//! (InstructionCombining.cpp) are not translated here. MIR's scalar alloca has
//! no count operand, so a count of 1 is dropping the operand.
//!
//! # Used by
//! `combine` (its `visit`).
//!
//! # Example
//! The first store is overwritten before anything reads it:
//! ```mir
//! store i16 1, ptr %p
//! store i16 2, ptr %p
//! ```
//! After, one store: `store i16 2, ptr %p`.

use llrm_mir::context::ConstantKind;
use llrm_mir::edit::Position;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{CastOp, Flags, Opcode};
use llrm_mir::types::Type;
use llrm_mir::valuetracking::nonzero::null_pointer_is_defined;

use crate::combine::{Combiner, Visit, defining};
use crate::combine_shared::{has_one_use, replace_operand};

/// How many instructions before a store the scan for an overwritten store
/// looks at.
const SCAN_INSTS: u32 = 6;

impl Combiner<'_> {
    /// The instruction that defines `v`, when it is in the body.
    fn instruction_of(
        &self,
        v: Operand,
    ) -> Option<InstId> {
        defining(self.function, v)
    }

    /// Whether `v` is the result of an alloca.
    fn is_alloca(
        &self,
        v: Operand,
    ) -> bool {
        self.instruction_of(v)
            .is_some_and(|inst| matches!(self.function.instruction(inst).opcode, Opcode::Alloca { .. }))
    }

    /// `equivalentAddressValues`: the same value, or arithmetic, a cast, a phi
    /// or a GEP that is the same instruction as `b` but for its flags.
    fn equivalent_address_values(
        &self,
        a: Operand,
        b: Operand,
    ) -> bool {
        if a == b {
            return true;
        }
        let (Some(a), Some(b)) = (self.instruction_of(a), self.instruction_of(b)) else { return false };
        let (a, b) = (self.function.instruction(a), self.function.instruction(b));
        matches!(
            a.opcode,
            Opcode::Binary(_) | Opcode::Cast(_) | Opcode::Phi | Opcode::GetElementPtr { .. }
        )
            && a.opcode == b.opcode
            && a.ty == b.ty
            && a.operands == b.operands
    }

    /// `canSimplifyNullStoreOrGEP`: the address is null, or a GEP of null, and
    /// null is not a defined address there.
    fn can_simplify_null_store_or_gep(
        &self,
        ptr: Operand,
    ) -> bool {
        let Type::Pointer(space) = *self.context.types.get(self.type_of(ptr)) else { return false };
        if null_pointer_is_defined(Some(self.function), space) {
            return false;
        }
        let base = match self.instruction_of(ptr) {
            Some(gep) if matches!(
                self.function.instruction(gep).opcode,
                Opcode::GetElementPtr { .. }
            ) => {
                self.function.instruction(gep).operands[0]
            }
            _ => ptr,
        };
        self.is_null_pointer(base)
    }

    /// Whether the scan before a store must stop at `inst`: it may read or
    /// write memory or throw (DEVIATE 5).
    fn ends_scan(
        &self,
        inst: InstId,
    ) -> bool {
        matches!(
            self.function.instruction(inst).opcode,
            Opcode::Load { .. } | Opcode::Store { .. } | Opcode::Call(_) | Opcode::Invoke(_) | Opcode::Resume
        )
    }

    /// `InstCombinerImpl::visitStoreInst`.
    pub fn visit_store(
        &mut self,
        inst: InstId,
    ) -> Visit {
        let Opcode::Store { volatile, .. } = self.function.instruction(inst).opcode else { return Visit::None };
        let [val, ptr] = self.function.instruction(inst).operands[..] else { return Visit::None };

        // Don't hack volatile/ordered stores.
        if volatile {
            return Visit::None;
        }

        // If the RHS is an alloca with a single use, zapify the store, making
        // the alloca dead.
        if has_one_use(self, ptr) {
            if self.is_alloca(ptr) {
                return self.erase_store(inst);
            }
            if let Some(gep) = self.instruction_of(ptr)
                && matches!(
                    self.function.instruction(gep).opcode,
                    Opcode::GetElementPtr { .. }
                )
                && self.is_alloca(self.function.instruction(gep).operands[0])
                && has_one_use(self, self.function.instruction(gep).operands[0])
            {
                return self.erase_store(inst);
            }
        }

        // Do really simple DSE, to catch cases where there are several
        // consecutive stores to the same location, separated by a few
        // arithmetic operations.
        let mut scan = SCAN_INSTS;
        let mut at = inst;
        while scan > 0 {
            let Some(before) = self.function.prev_in_block(at) else { break };
            at = before;
            scan -= 1;
            let prior = self.function.instruction(before);
            if let Opcode::Store { volatile: false, .. } = prior.opcode {
                if self.equivalent_address_values(prior.operands[1], ptr)
                    && self.type_of(prior.operands[0]) == self.type_of(val)
                {
                    // The original store goes back on the worklist after the
                    // operands of the removed one.
                    self.worklist.push(inst);
                    self.erase(before);
                    return Visit::Changed;
                }
                break;
            }
            // `X = load P; store X -> P`: this store is dead.
            if matches!(prior.opcode, Opcode::Load { .. }) {
                if val == Operand::Value(prior.result.expect("a load's value"))
                    && self.equivalent_address_values(prior.operands[0], ptr)
                {
                    return self.erase_store(inst);
                }
                break;
            }
            if self.ends_scan(before) {
                break;
            }
        }

        // store X, null    -> turns into 'unreachable' in SimplifyCFG
        // store X, GEP(null, Y) -> turns into 'unreachable' in SimplifyCFG
        if self.can_simplify_null_store_or_gep(ptr) {
            if !self.is_poison_value(val) {
                let ty = self.type_of(val);
                let poison = self.poison(ty);
                replace_operand(self, inst, 0, poison);
                return Visit::Changed;
            }
            return Visit::None;
        }

        // store undef, Ptr -> noop
        if self.is_undef_value(val) {
            return self.erase_store(inst);
        }

        Visit::None
    }

    /// `AllocaInst::getAllocationSize`: the bytes it allocates, when its count
    /// is a number.
    fn allocation_size(
        &self,
        inst: InstId,
    ) -> Option<u64> {
        let instruction = self.function.instruction(inst);
        let Opcode::Alloca { allocated, .. } = instruction.opcode else { return None };
        let size = self.env.layout.alloc_size(&self.context.types, allocated);
        match instruction.operands.first() {
            None => Some(size),
            Some(&Operand::Constant(count)) => match self.context.get(count).kind {
                ConstantKind::Int(count) => u64::try_from(count).ok()?.checked_mul(size),
                _ => None,
            },
            Some(_) => None,
        }
    }

    /// `simplifyAllocaArraySize`: a count of 1 is no count, a number is an
    /// array type, `undef` is poison, any other count has the width of an
    /// index. Without a count (MIR's scalar alloca) there is nothing to do.
    fn simplify_alloca_array_size(
        &mut self,
        inst: InstId,
    ) -> Option<Visit> {
        let instruction = self.function.instruction(inst);
        let &count = instruction.operands.first()?;
        let ptr_ty = instruction.ty;
        let Opcode::Alloca { allocated, align, address_space } = instruction.opcode else { return None };
        let flags = instruction.flags;
        if let Operand::Constant(id) = count
            && let ConstantKind::Int(bits) = self.context.get(id).kind
            && let Ok(bits) = u64::try_from(bits)
        {
            if bits == 1 {
                self.function.set_operands(inst, Vec::new());
                return Some(Visit::Changed);
            }
            let array = self.context.types.intern(Type::Array { element: allocated, count: bits });
            let new = Opcode::Alloca { allocated: array, align, address_space };
            return Some(self.replace_with_new(inst, new, ptr_ty, Vec::new(), flags));
        }
        if self.is_undef_value(count) {
            return Some(Visit::Replace(self.poison(ptr_ty)));
        }
        let index = self.context.types.int(self.env.layout.pointer(address_space).index_bits);
        let from = self.type_of(count);
        if from == index {
            return None;
        }
        let (Type::Int(from_bits), Type::Int(to_bits)) =
            (self.context.types.get(from).clone(), self.context.types.get(index).clone())
        else {
            return None;
        };
        let op = if from_bits > to_bits { CastOp::Trunc } else { CastOp::ZExt };
        let cast = self.create_cast(inst, op, count, index, Flags::default());
        replace_operand(self, inst, 0, cast);
        Some(Visit::Changed)
    }

    /// `InstCombinerImpl::visitAllocaInst`.
    pub fn visit_alloca(
        &mut self,
        inst: InstId,
    ) -> Visit {
        if let Some(visit) = self.simplify_alloca_array_size(inst) {
            return visit;
        }

        // Move all alloca's of zero byte objects to the entry block and merge
        // them together.
        if self.allocation_size(inst) == Some(0) {
            let entry = self.function.entry().expect("a body");
            let first = self
                .function
                .iter_block(entry)
                .find(|&one| self.function.instruction(one).opcode != Opcode::Phi)
                .expect("a terminator");
            if first != inst {
                let is_zero_alloca = self.allocation_size(first) == Some(0)
                    && matches!(self.function.instruction(first).opcode, Opcode::Alloca { .. });
                if !is_zero_alloca {
                    self.function.move_to(inst, Position::Before(first)).expect("a placed alloca");
                    return Visit::Changed;
                }
                // Merge into the one at the start, aligned for both.
                let (
                    Opcode::Alloca { allocated: a, align: a_align, .. },
                    Opcode::Alloca { allocated: b, align: b_align, .. },
                ) = (self.function.instruction(first).opcode.clone(), self.function.instruction(inst).opcode.clone())
                else {
                    unreachable!("two allocas")
                };
                let layout = self.env.layout;
                let align = a_align
                    .unwrap_or_else(|| layout.align(&self.context.types, a))
                    .max(b_align.unwrap_or_else(|| layout.align(&self.context.types, b)));
                self.function.set_alloca_align(first, align);
                let merged = Operand::Value(self.function.instruction(first).result.expect("a value"));
                return Visit::Replace(merged);
            }
        }

        // DEVIATE 6 is here: isOnlyCopiedFromConstantMemory, then
        // visitAllocSite.
        Visit::None
    }

    /// `eraseInstFromFunction(SI)` as what `visit*` returns.
    fn erase_store(
        &mut self,
        inst: InstId,
    ) -> Visit {
        self.erase(inst);
        Visit::Erased
    }
}

#[cfg(test)]
#[path = "combine_loadstore_tests.rs"]
mod tests;
