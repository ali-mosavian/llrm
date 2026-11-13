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
//! `mergeStoreIntoSuccessor` (:1639, called from `visitUncondBrInst`): a store
//! before an unconditional branch whose successor has two edges, and the
//! other predecessor stores the same address (last in an if/else arm, or in
//! the block that branches to this one: the triangle), becomes a `storemerge`
//! phi and one store in the successor.
//!
//! DEVIATE 9: only stores of one type merge (no `CreateBitOrPointerCast`
//! between a stored `i32` and `float`); of the metadata only an equal `tbaa`
//! is kept (`AAMDNodes::merge` and the access-group intersection are not
//! translated). Exit 2026-11-20.
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
//! DEVIATE 6 is on `replace_alloca_copied_from_constant`
//! (`isOnlyCopiedFromConstantMemory`; `PointerReplacer` is not needed, there
//! is one address space); `visitAllocSite` has DEVIATE 7 on
//! `alloc_site_removable`. MIR's scalar alloca has no count operand, so a
//! count of 1 is dropping the operand.
//!
//! `visitLoadInst`'s store-to-load forwarding: a load whose value one of the 6
//! instructions before it in its block stored (`FindAvailableLoadedValue`) is
//! that value, cast to the load's type.
//!
//! `combineLoadToOperationType` (:691): a load read once by a `bitcast` loads
//! the cast's type.
//!
//! DEVIATE 8: the rest of `visitLoadInst` is not translated, and the load CSE
//! of `FindAvailableLoadedValue` (an earlier load of the address) is left to
//! GVN: the combine holds no metadata to merge (`combineMetadataForCSE`).
//! The alias analysis is BasicAA with its dominator tree off, not the
//! pipeline's chain; the stores skipped are those it proves disjoint. Exit:
//! the rest of `visitLoadInst`.
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

use llrm_analysis::aa::BatchAAResults;
use llrm_analysis::basicaa::BasicAA;
use llrm_analysis::loads::{Available, DEF_MAX_INSTS_TO_SCAN, find_available_loaded_value_before};
use llrm_analysis::memory::Unit;
use llrm_mir::context::ConstantKind;
use llrm_mir::edit::Position;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{CastOp, Flags, Opcode};
use llrm_mir::types::Type;
use llrm_mir::valuetracking::nonzero::null_pointer_is_defined;

use crate::combine::{Combiner, Visit, defining};
use crate::combine_shared::{has_one_use, replace_operand};

/// `MaxCopiedFromConstantUsers`.
const MAX_COPIED_FROM_CONSTANT_USERS: usize = 300;

/// `MaxAllocSiteRemovableUsers`.
const MAX_ALLOC_SITE_REMOVABLE_USERS: usize = 128;

/// How many instructions before a store the scan for an overwritten store
/// looks at.
const SCAN_INSTS: u32 = 6;

/// The kinds `copyMetadataForLoad` copies as they are (`dbg` is the builder's).
const COPIED_LOAD_METADATA: &[&str] = &[
    "tbaa",
    "prof",
    "fpmath",
    "tbaa_struct",
    "invariant.load",
    "alias.scope",
    "noalias",
    "nontemporal",
    "mem.cache_hint",
    "llvm.mem.parallel_loop_access",
    "llvm.access.group",
    "noundef",
    "noalias_addrspace",
    "invariant.group",
];

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
            && a.is_identical_to_when_defined(b)
    }

    /// `combineLoadToOperationType`
    /// ([:691](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineLoadStoreAlloca.cpp#L691)):
    /// a load read once, by a `bitcast`, loads the cast's type instead. A
    /// pointer cast to an integer is punning and stays. Metadata that holds
    /// for any type is copied (`copyMetadataForLoad`); `range`, `nonnull`,
    /// `align`, `dereferenceable` and `nofpclass` describe the old type and
    /// are dropped (no pointer or same-type case reaches here).
    fn combine_load_to_operation_type(
        &mut self,
        load: InstId,
    ) -> Visit {
        let Opcode::Load { align, volatile: false } = self.function.instruction(load).opcode else {
            return Visit::None;
        };
        let Some(result) = self.function.instruction(load).result else { return Visit::None };
        let users = self.function.users(result);
        if !users.has_one_use() {
            return Visit::None;
        }
        let Some(cast) = users.iter().next().map(|one| one.user) else { return Visit::None };
        let Opcode::Cast(CastOp::BitCast) = self.function.instruction(cast).opcode else { return Visit::None };
        let ty = self.function.instruction(cast).ty;
        let is_pointer = |c: &Self, ty| matches!(c.context.types.get(ty), Type::Pointer(_));
        if is_pointer(self, ty) != is_pointer(self, self.function.instruction(load).ty) {
            return Visit::None;
        }
        let pointer = self.function.instruction(load).operands.clone();
        let metadata = self.function.instruction(load).metadata.clone();
        let new = self.insert_before(load, Opcode::Load { align, volatile: false }, ty, pointer, Flags::default());
        for (kind, node) in metadata {
            if COPIED_LOAD_METADATA.contains(&kind.as_str()) {
                self.function.annotate(new, &kind, node);
            }
        }
        let new_value = self.value_of(new);
        self.replace_inst_with(cast, new_value);
        self.erase(cast);
        self.erase(load);
        Visit::Erased
    }

    /// `visitLoadInst`: `combineLoadToOperationType` and the store-to-load
    /// forwarding (DEVIATE 8).
    pub fn visit_load(
        &mut self,
        load: InstId,
    ) -> Visit {
        match self.combine_load_to_operation_type(load) {
            Visit::None => {}
            done => return done,
        }
        let available = {
            let context: &llrm_mir::context::Context = &*self.context;
            let function = &*self.function;
            let unit = Unit::over(context, self.env.layout, &[], self.env.globals, function);
            let mut basic = BasicAA::new(&unit);
            let mut batch = BatchAAResults::new(&mut basic);
            batch.disable_dominator_tree();
            find_available_loaded_value_before(&unit, load, &mut batch, DEF_MAX_INSTS_TO_SCAN)
        };
        let Some(Available { value, is_load_cse: false }) = available else { return Visit::None };
        let ty = self.function.instruction(load).ty;
        // The load itself only in a dead loop.
        if self.function.instruction(load).result.is_some_and(|result| value == Operand::Value(result)) {
            return Visit::Replace(self.poison(ty));
        }
        let from = self.type_of(value);
        if from == ty {
            return Visit::Replace(value);
        }
        let op = match (self.context.types.get(from), self.context.types.get(ty)) {
            (Type::Pointer(_), Type::Int(_)) => CastOp::PtrToInt,
            (Type::Int(_), Type::Pointer(_)) => CastOp::IntToPtr,
            (Type::Pointer(_), Type::Pointer(_)) => CastOp::AddrSpaceCast,
            _ => CastOp::BitCast,
        };
        Visit::Replace(self.create_cast(load, op, value, ty, Flags::default()))
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

    /// `InstCombinerImpl::visitUncondBrInst`: the store just before an
    /// unconditional branch, merged into the successor if it can be.
    pub fn visit_uncond_br(
        &mut self,
        inst: InstId,
    ) -> Visit {
        if self.function.instruction(inst).operands.len() != 1 {
            return Visit::None;
        }
        let Some(store) = self.function.prev_in_block(inst) else { return Visit::None };
        if !matches!(self.function.instruction(store).opcode, Opcode::Store { .. }) {
            return Visit::None;
        }
        if self.merge_store_into_successor(store) { Visit::Changed } else { Visit::None }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineLoadStoreAlloca.cpp#L1639
    /// `mergeStoreIntoSuccessor`: `if () { *P = v1; } else { *P = v2 }` or
    /// `*P = v1; if () { *P = v2; }` becomes a phi and one store in the
    /// successor.
    fn merge_store_into_successor(
        &mut self,
        store: InstId,
    ) -> bool {
        let Opcode::Store { volatile: false, .. } = self.function.instruction(store).opcode else { return false };
        let [value, ptr] = self.function.instruction(store).operands[..] else { return false };
        let store_bb = self.function.parent(store).expect("a placed store");
        let Some(&Operand::Block(dest_bb)) =
            self.function.instruction(self.function.terminator(store_bb).expect("terminated")).operands.first()
        else {
            return false;
        };
        // The successor has exactly two incoming edges.
        let preds = self.function.predecessors(dest_bb);
        if preds.len() != 2 {
            return false;
        }
        let Some(&other_bb) = preds.iter().find(|&&one| one != store_bb) else { return false };
        // The blocks must be distinct, and the other one not empty apart from
        // its terminator.
        if store_bb == dest_bb || other_bb == dest_bb {
            return false;
        }
        let other_term = self.function.terminator(other_bb).expect("terminated");
        if self.function.prev_in_block(other_term).is_none() {
            return false;
        }
        let mergeable = |c: &Self, other: InstId| {
            let one = c.function.instruction(other);
            one.opcode == c.function.instruction(store).opcode
                && one.operands[1] == ptr
                && c.type_of(one.operands[0]) == c.type_of(value)
        };
        let other_store = if self.function.instruction(other_term).operands.len() == 1 {
            // If the other block ends in an unconditional branch, this is the
            // 'if then else' case: the store is the instruction before it.
            let before = self.function.prev_in_block(other_term).expect("not empty");
            if !mergeable(self, before) {
                return false;
            }
            before
        } else {
            // Otherwise the other block ended with a conditional branch to
            // `store_bb` and `dest_bb`, the if/then triangle.
            let targets = &self.function.instruction(other_term).operands[1..];
            if !targets.contains(&Operand::Block(store_bb)) {
                return false;
            }
            let mut at = other_term;
            let found = loop {
                if let Some(before) = self.function.prev_in_block(at) {
                    at = before;
                } else {
                    return false;
                }
                if matches!(self.function.instruction(at).opcode, Opcode::Store { .. }) && mergeable(self, at) {
                    break at;
                }
                if self.touches_memory_or_throws(at) {
                    return false;
                }
            };
            // Nothing may read or overwrite the stored value in `store_bb`.
            let mut one = self.function.iter_block(store_bb).next().expect("not empty");
            while one != store {
                if self.touches_memory_or_throws(one) {
                    return false;
                }
                one = self.function.next_in_block(one).expect("the store is in its block");
            }
            found
        };

        let other_value = self.function.instruction(other_store).operands[0];
        let merged = if other_value != value {
            let ty = self.type_of(value);
            let first = self.function.iter_block(dest_bb).next().expect("not empty");
            let operands = vec![value, Operand::Block(store_bb), other_value, Operand::Block(other_bb)];
            let phi = self.insert_before(first, Opcode::Phi, ty, operands, Flags::default());
            let result = self.function.instruction(phi).result.expect("a value");
            self.function.set_name(result, "storemerge");
            Operand::Value(result)
        } else {
            other_value
        };
        let at = self.first_insertion_point(dest_bb);
        let void = self.context.types.void();
        let opcode = self.function.instruction(store).opcode.clone();
        let new = self.insert_before(at, opcode, void, vec![merged, ptr], Flags::default());
        // Metadata both stores carry alike stays; DEVIATE 9.
        let kept: Vec<_> = self
            .function
            .instruction(store)
            .metadata
            .iter()
            .filter(|(kind, node)| {
                kind == "tbaa"
                    && self.function.instruction(other_store).metadata.iter().any(|(k, n)| k == kind && n == node)
            })
            .cloned()
            .collect();
        for (kind, node) in kept {
            self.function.set_metadata(new, &kind, Some(node));
        }
        self.erase(store);
        self.erase(other_store);
        true
    }

    /// `mayReadFromMemory() || mayThrow() || mayWriteToMemory()`.
    fn touches_memory_or_throws(
        &self,
        inst: InstId,
    ) -> bool {
        self.ends_scan(inst) || matches!(self.function.instruction(inst).opcode, Opcode::Atomic(_))
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

        if let Some(visit) = self.replace_alloca_copied_from_constant(inst) {
            return visit;
        }
        self.visit_alloc_site(inst)
    }

    /// `visitAllocaInst`'s `isOnlyCopiedFromConstantMemory` block
    /// ([InstCombineLoadStoreAlloca.cpp:556](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineLoadStoreAlloca.cpp#L556)):
    /// an alloca only read after one `memcpy` from a constant global is that
    /// global, and the copy and the lifetime markers go.
    ///
    /// DEVIATE 6: the source is a constant global itself (not a constant
    /// expression of one); its alignment is not raised
    /// (`getOrEnforceKnownAlignment`); a call that takes the alloca as an
    /// argument keeps the alloca. Exit: constant-GEP sources, then the
    /// `nocapture readonly` arguments of `isOnlyCopiedFromConstantMemory`.
    fn replace_alloca_copied_from_constant(
        &mut self,
        alloca: InstId,
    ) -> Option<Visit> {
        let (copy, markers) = self.only_copied_from_constant_memory(alloca)?;
        let source = self.function.instruction(copy).operands[1];
        let Opcode::Alloca { allocated, align, .. } = self.function.instruction(alloca).opcode else { return None };
        let layout = self.env.layout;
        let alloca_align = align.unwrap_or_else(|| layout.align(&self.context.types, allocated));
        let Operand::Constant(id) = source else { return None };
        let llrm_mir::context::ConstantKind::Global(global) = self.context.get(id).kind else { return None };
        let llrm_mir::module::GlobalKind::Variable(variable) = &self.env.globals[global.0 as usize].kind else {
            return None;
        };
        let source_align = variable.align.unwrap_or_else(|| layout.align(&self.context.types, variable.ty));
        if alloca_align > source_align
            || layout.alloc_size(&self.context.types, variable.ty) < self.allocation_size(alloca)?
        {
            return None;
        }
        for marker in markers {
            self.erase(marker);
        }
        self.erase(copy);
        Some(Visit::Replace(source))
    }

    /// `isOnlyCopiedFromConstantMemory`
    /// ([:46](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineLoadStoreAlloca.cpp#L46)):
    /// the one `memcpy`/`memmove` that writes the alloca, whose source is
    /// constant memory, and the lifetime markers, when nothing else writes
    /// it or lets it escape: only loads, `memcpy` sources, and pointer
    /// arithmetic and phis and selects of it. A copy through an offset
    /// pointer, or after a phi or select, is rejected.
    fn only_copied_from_constant_memory(
        &self,
        alloca: InstId,
    ) -> Option<(InstId, Vec<InstId>)> {
        use llrm_mir::intrinsics::Intrinsic;
        let (mut copy, mut markers) = (None, Vec::new());
        let mut visited: Vec<(llrm_mir::module::ValueId, bool)> = Vec::new();
        let mut work = vec![(self.function.instruction(alloca).result?, false)];
        while let Some((value, is_offset)) = work.pop() {
            if visited.contains(&(value, is_offset)) {
                continue;
            }
            visited.push((value, is_offset));
            if visited.len() > MAX_COPIED_FROM_CONSTANT_USERS {
                return None;
            }
            for one in self.function.users(value).iter() {
                let instruction = self.function.instruction(one.user);
                let result = instruction.result;
                match instruction.opcode {
                    Opcode::Load { volatile: false, .. } => {}
                    Opcode::Phi | Opcode::Select => work.push((result?, true)),
                    Opcode::Cast(CastOp::AddrSpaceCast) => work.push((result?, is_offset)),
                    Opcode::GetElementPtr { .. } => {
                        let zero = instruction.operands[1..].iter().all(|&index| {
                            matches!(
                                index,
                                Operand::Constant(id)
                                    if matches!(self.context.get(id).kind, ConstantKind::Int(0) | ConstantKind::Zero)
                            )
                        });
                        work.push((result?, is_offset || !zero));
                    }
                    Opcode::Call(_) => match self.intrinsic_of(one.user)? {
                        Intrinsic::LifetimeStart | Intrinsic::LifetimeEnd => markers.push(one.user),
                        Intrinsic::MemCpy | Intrinsic::MemMove => {
                            let volatile = match instruction.operands.get(3) {
                                Some(Operand::Constant(id)) => {
                                    !matches!(
                                        self.context.get(*id).kind,
                                        ConstantKind::Int(0) | ConstantKind::Zero
                                    )
                                }
                                _ => true,
                            };
                            if volatile {
                                return None;
                            }
                            // The alloca as the source is a load.
                            if one.index == 1 {
                                continue;
                            }
                            if copy.is_some() || is_offset || one.index != 0 {
                                return None;
                            }
                            if !self.is_constant_global(instruction.operands[1]) {
                                return None;
                            }
                            copy = Some(one.user);
                        }
                        _ => return None,
                    },
                    _ => return None,
                }
            }
        }
        Some((copy?, markers))
    }

    /// `pointsToConstantMemory` of a plain global: a constant that the linker
    /// cannot replace and nothing initializes later.
    fn is_constant_global(
        &self,
        pointer: Operand,
    ) -> bool {
        let Operand::Constant(id) = pointer else { return false };
        let ConstantKind::Global(global) = self.context.get(id).kind else { return false };
        let value = &self.env.globals[global.0 as usize];
        let llrm_mir::module::GlobalKind::Variable(variable) = &value.kind else { return false };
        variable.constant
            && variable.initializer.is_some()
            && !variable.externally_initialized
            && !matches!(
                value.linkage,
                llrm_mir::module::Linkage::Weak
                    | llrm_mir::module::Linkage::LinkOnce
                    | llrm_mir::module::Linkage::Common
                    | llrm_mir::module::Linkage::ExternWeak
            )
    }

    /// `InstCombinerImpl::visitAllocSite` of an alloca
    /// ([InstructionCombining.cpp:3900](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L3900)):
    /// an alloca that is only written, or only read (its initial value is
    /// undef), is erased with its users: a load reads undef, a pointer made of
    /// it is poison.
    fn visit_alloc_site(
        &mut self,
        alloca: InstId,
    ) -> Visit {
        let Some(users) = self.alloc_site_removable(alloca) else { return Visit::None };
        // The values the users made are replaced first, then each user is
        // erased before the casts and GEPs it was reached through.
        for &user in &users {
            let instruction = self.function.instruction(user);
            let Some(result) = instruction.result else { continue };
            let ty = instruction.ty;
            let kind = if matches!(instruction.opcode, Opcode::Load { .. }) {
                ConstantKind::Undef
            } else {
                ConstantKind::Poison
            };
            let replacement = Operand::Constant(self.context.constant(llrm_mir::context::Constant { ty, kind }));
            self.function.replace_value(result, replacement);
        }
        for user in users.into_iter().rev() {
            self.erase(user);
        }
        self.erase(alloca);
        Visit::Erased
    }

    /// The intrinsic `call` calls.
    pub(crate) fn intrinsic_of(
        &self,
        call: InstId,
    ) -> Option<llrm_mir::intrinsics::Intrinsic> {
        let callee = llrm_mir::memory::callee(self.context, self.function, call)?;
        self.env.globals.get(callee.0 as usize)?.intrinsic()
    }

    /// `isAllocSiteRemovable`
    /// ([InstructionCombining.cpp:3742](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L3742)):
    /// the users of `alloca` and of the casts and GEPs of it, in the order they
    /// were reached, when every one is a non-volatile store to it, a
    /// non-volatile load from it, a `memset`/`memcpy`/`memmove` with it as
    /// destination or as source, or a lifetime marker or `assume`. The accesses
    /// must all write or all read (`Access`): a read after a write, or a write
    /// after a read, keeps the alloca.
    ///
    /// DEVIATE 7: not made: `icmp` equalities with null or another allocation,
    /// `objectsize`, `invariant.start`/`end`, `launder.invariant.group`, and
    /// the calls whose only effect is writing to the alloca
    /// (`isRemovableWrite`); an alloca with one of those stays. Exit: a
    /// slice after the C front end's `memcpy` lands (#3621).
    fn alloc_site_removable(
        &self,
        alloca: InstId,
    ) -> Option<Vec<InstId>> {
        use llrm_mir::intrinsics::Intrinsic;
        let mut users: Vec<InstId> = Vec::new();
        let mut work = vec![alloca];
        // `Access`: Mod and Ref.
        let (mut modified, mut referenced) = (false, false);
        while let Some(address) = work.pop() {
            let value = self.function.instruction(address).result?;
            for one in self.function.users(value).iter() {
                if users.len() >= MAX_ALLOC_SITE_REMOVABLE_USERS {
                    return None;
                }
                let instruction = self.function.instruction(one.user);
                match instruction.opcode {
                    Opcode::Cast(CastOp::AddrSpaceCast) | Opcode::GetElementPtr { .. } => {
                        if !users.contains(&one.user) {
                            users.push(one.user);
                            work.push(one.user);
                        }
                    }
                    Opcode::Store { volatile: false, .. } if one.index == 1 => {
                        if referenced {
                            return None;
                        }
                        modified = true;
                        if !users.contains(&one.user) {
                            users.push(one.user);
                        }
                    }
                    Opcode::Load { volatile: false, .. } if one.index == 0 => {
                        if modified {
                            return None;
                        }
                        referenced = true;
                        if !users.contains(&one.user) {
                            users.push(one.user);
                        }
                    }
                    Opcode::Call(_) => match self.intrinsic_of(one.user)? {
                        Intrinsic::MemSet | Intrinsic::MemCpy | Intrinsic::MemMove => {
                            let volatile = match instruction.operands.get(3) {
                                Some(Operand::Constant(id)) => {
                                    !matches!(
                                        self.context.get(*id).kind,
                                        ConstantKind::Int(0) | ConstantKind::Zero
                                    )
                                }
                                _ => true,
                            };
                            if volatile {
                                return None;
                            }
                            if instruction.operands.first() == Some(&Operand::Value(value)) {
                                if referenced {
                                    return None;
                                }
                                modified = true;
                            } else {
                                if modified {
                                    return None;
                                }
                                referenced = true;
                            }
                            if !users.contains(&one.user) {
                                users.push(one.user);
                            }
                        }
                        Intrinsic::LifetimeStart
                        | Intrinsic::LifetimeEnd
                        | Intrinsic::InvariantStart
                        | Intrinsic::InvariantEnd
                        | Intrinsic::Assume => {
                            if !users.contains(&one.user) {
                                users.push(one.user);
                            }
                        }
                        _ => return None,
                    },
                    _ => return None,
                }
            }
        }
        Some(users)
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
