//! InstCombine's stores: the store that cannot matter is erased, the one that
//! cannot execute stores poison. Translated from `visitStoreInst`
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

use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::module::{InstId, Operand, ValueDef};
use llrm_mir::opcode::Opcode;
use llrm_mir::types::Type;
use llrm_mir::valuetracking::nonzero::null_pointer_is_defined;

use crate::combine::{Combiner, Visit};

/// How many instructions before a store the scan for an overwritten store
/// looks at.
const SCAN_INSTS: u32 = 6;

impl Combiner<'_> {
    /// `isa<UndefValue>`: `undef` or `poison`.
    fn is_undef_value(
        &self,
        v: Operand,
    ) -> bool {
        matches!(
            v,
            Operand::Constant(id) if matches!(self.context.get(id).kind, ConstantKind::Undef | ConstantKind::Poison)
        )
    }

    /// `isa<PoisonValue>`.
    fn is_poison_value(
        &self,
        v: Operand,
    ) -> bool {
        matches!(
            v,
            Operand::Constant(id) if self.context.get(id).kind == ConstantKind::Poison
        )
    }

    /// `isa<ConstantPointerNull>`.
    fn is_null(
        &self,
        v: Operand,
    ) -> bool {
        matches!(
            v,
            Operand::Constant(id) if matches!(self.context.get(id).kind, ConstantKind::Null | ConstantKind::Zero)
        )
    }

    /// The instruction that defines `v`, when it is in the body.
    fn instruction_of(
        &self,
        v: Operand,
    ) -> Option<InstId> {
        let Operand::Value(value) = v else { return None };
        let ValueDef::Instruction(inst) = self.function.value(value).def else { return None };
        self.function.parent(inst).map(|_| inst)
    }

    /// Whether `v` is the result of an alloca.
    fn is_alloca(
        &self,
        v: Operand,
    ) -> bool {
        self.instruction_of(v)
            .is_some_and(|inst| matches!(self.function.instruction(inst).opcode, Opcode::Alloca { .. }))
    }

    /// `Value::hasOneUse`.
    pub(crate) fn has_one_use(
        &self,
        v: Operand,
    ) -> bool {
        let Operand::Value(value) = v else { return false };
        self.function.users(value).has_one_use()
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
            && a.is_identical_to_when_defined(b)
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
        self.is_null(base)
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
        if self.has_one_use(ptr) {
            if self.is_alloca(ptr) {
                return self.erase_store(inst);
            }
            if let Some(gep) = self.instruction_of(ptr)
                && matches!(
                    self.function.instruction(gep).opcode,
                    Opcode::GetElementPtr { .. }
                )
                && self.is_alloca(self.function.instruction(gep).operands[0])
                && self.has_one_use(self.function.instruction(gep).operands[0])
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
                let poison = Operand::Constant(self.context.constant(Constant { ty, kind: ConstantKind::Poison }));
                self.function.set_operand(inst, 0, poison);
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
