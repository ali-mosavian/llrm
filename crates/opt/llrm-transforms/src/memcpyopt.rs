//! MemCpyOpt on MemorySSA: LLVM's `MemCpyOptPass` apart from the memset
//! merging of `processStore` and `processMemSet`, which
//! [`crate::memcpyoptimizer`] does (stage 1). A translation of
//! `MemCpyOptimizer.cpp` at d1106deb71cc.
//!
//! # How it works
//! Each memcpy, memmove and aggregate store of a load is looked at in block
//! order with the memory graph (`mssa`) kept right by the updater as
//! instructions are made and erased. The clobber a memcpy's source or
//! destination walks up to decides what it can become:
//! - a copy to itself, or of no bytes, is gone; a copy out of a constant global
//!   that repeats one byte is a memset;
//! - a memcpy of a memcpy's destination copies from that memcpy's source
//!   instead (`processMemCpyMemCpyDependence`);
//! - a memcpy right after a memset of its destination: the memset shrinks to
//!   what the copy leaves (`processMemSetMemCpyDependence`);
//! - a memcpy of a memset's bytes is a memset (`performMemCpyToMemSetOptzn`);
//! - a copy of memory nothing wrote (a fresh alloca) is gone
//!   (`hasUndefContents`);
//! - a call that fills a fresh alloca, copied to another place, fills that
//!   place instead and the copy is gone (`performCallSlotOptzn`, from a memcpy
//!   or a load and store of the alloca);
//! - a copy of a whole stack slot to another that never conflict with it makes
//!   the two one slot (`performStackMoveOptzn`);
//! - a `byval` argument, or one a callee only reads, fed by a memcpy takes the
//!   memcpy's source (`processByValArgument`, `processImmutArgument`);
//! - a memmove whose ranges cannot meet is a memcpy (`processMemMove`);
//! - an aggregate loaded and stored right after is a memcpy, or a memmove where
//!   the store may write what the load reads (`processStoreOfLoad`), the store
//!   lifted above the first write to the loaded bytes when nothing in between
//!   needs it (`moveUp`).
//!
//! # Example
//! A copy of a copy reads the first source:
//!
//! ```text
//!  call memcpy(%b, %a, 16)           call memcpy(%b, %a, 16)
//!  call memcpy(%c, %b, 16)    ==>    call memcpy(%c, %a, 16)
//! ```
//!
//! # DEVIATEs
//! 1. Alias questions are `mssa::alias`'s (llrm's reference model), not
//!    BasicAA's, and there is no `EarliestEscapeAnalysis`: what it adds to
//!    `BatchAAResults` is a capture query the reference model answers by the
//!    object's exposure. Exit: none; the model is llrm's.
//! 2. Nothing throws (no unwinding in MIR): `mayBeVisibleThroughUnwinding` is
//!    false.
//! 3. No debug-info assignment IDs (`mergeDIAssignID`), no `combineAAMetadata`
//!    (the formed calls carry no `!tbaa`, as in `loopidiom`).
//! 5. MIR holds no constant expression in code: where the builder folds
//!    `getelementptr (i8, @g, 16)` the pass makes the instruction. A forced
//!    inline copy (`llvm.memcpy.inline`) is not forwarded, as the builder's
//!    `CreateMemCpyInline` has no counterpart here.
//! 9. `getOrEnforceKnownAlignment` is `known_alignment`: it never raises an
//!    alloca's or global's alignment, so a source aligned less than the byval
//!    or the alloca needs ends the forwarding. Exit: the alignment-raising
//!    helper (needs the writer of alloca alignment shared with call slot).
//! 6. `moveUp` does not lift a call (LLVM does when it neither reads nor writes
//!    what the lifted ones touch): a call between the load and the store ends
//!    the attempt. Fact: `getModRefInfo(Instruction, CallBase)` has no model
//!    here. Exit: a call-against-call mod/ref query.
//! 7. The call slot asks `PointerMayBeCaptured` of the destination object where
//!    the pin asks `PointerMayBeCapturedBefore` the call (only when the callee
//!    may capture the source): a capture after the call also stops it. Exit: a
//!    dominance-aware capture query.
//! 8. The call slot gives up where the alias model says the call may touch the
//!    destination; the pin goes on to `callCapturesBefore`. Exit: that query in
//!    `llrm_analysis::aa`'s model.
//!
//! # Runs
//! In the slot of `memcpyopt@818`.
//!
//! # Keeps
//! The blocks and edges: only instructions change.
//!
//! # See also
//! [`crate::memcpyoptimizer`], [`crate::deadstore`], the memory graph in
//! `llrm_analysis::mssa`.

use std::rc::Rc;

use llrm_analysis::cfg::Shape;
use llrm_analysis::memory::{MemRef, Unit as View};
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::mssa::alias::{AliasQuery, Location};
use llrm_analysis::mssa::updater::MemorySsaUpdater;
use llrm_analysis::mssa::{AccessId, Kind, Mssa};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdMap;
use llrm_mir::domtree::DomTree;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Function, GlobalKind, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, Declared, Dominators, FunctionPass, Loops, Outer, PreservedAnalyses, Unit};
use llrm_mir::types::TypeId;
use llrm_mir::valuetracking::bytewise::{Byte, is_bytewise_value};
use llrm_mir::valuetracking::nonzero::{get_pointer_offset_from, is_known_non_zero};
use llrm_mir::valuetracking::{Query, underlying::get_underlying_object};

use crate::licm::apply::{Created, Graph};
use crate::loopidiom::build::{Copy, create_mem_copy, create_mem_set};

pub struct MemCpyOpt;

impl FunctionPass for MemCpyOpt {
    fn name(&self) -> &'static str {
        "memcpyopt"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("memcpyopt")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = Rc::clone(analyses.outer());
        let shape = analyses.get::<Shape>(&*unit.context, unit.layout, &*unit.function);
        let registers = analyses.get::<llrm_analysis::manager::Registers>(&*unit.context, unit.layout, &*unit.function);
        let dt = analyses.get::<Dominators>(&*unit.context, unit.layout, &*unit.function);
        let held = analyses.get::<llrm_analysis::manager::MemorySsa>(&*unit.context, unit.layout, &*unit.function);
        let made = match &*held {
            Ok(held) => held
                .accesses(&*unit.context, unit.layout, &*unit.function, analyses)
                .ok()
                .map(|accesses| (held.mssa.clone(), (*accesses).clone())),
            Err(_) => None,
        };
        let (mssa, accesses) = match made {
            Some(made) => made,
            None => {
                let memory = View::within(&*unit.context, unit.layout, &*unit.function, &outer)
                    .with_registers(&registers)
                    .with_shape(&shape);
                llrm_analysis::mssa::shadow::translated(&memory)
            }
        };
        let mut state = State {
            context: &mut *unit.context,
            layout: unit.layout,
            function: &mut *unit.function,
            declared: &mut *unit.declared,
            metadata: unit.metadata,
            outer,
            shape,
            registers,
            dt,
            mssa,
            accesses,
            updater: MemorySsaUpdater::new(),
        };
        if state.run_impl() {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// The kind of memory intrinsic a call is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind3 {
    Set,
    Copy,
    Move,
}

/// A `MemIntrinsic`: `llvm.memset`, `llvm.memcpy` or `llvm.memmove`.
#[derive(Clone, Copy, Debug)]
struct Mem {
    inst: InstId,
    kind: Kind3,
    /// `getRawDest`.
    dest: Operand,
    /// `getRawSource` of a transfer, `getValue` of a set.
    other: Operand,
    length: Operand,
    volatile: bool,
    /// `llvm.memcpy.inline`.
    force_inlined: bool,
}

struct State<'a> {
    context: &'a mut Context,
    layout: &'a DataLayout,
    function: &'a mut Function,
    declared: &'a mut Declared,
    metadata: &'a [llrm_mir::module::MetadataNode],
    outer: Rc<Outer>,
    shape: Rc<Shape>,
    registers: Rc<<llrm_analysis::manager::Registers as llrm_mir::passes::Analysis>::Result>,
    dt: Rc<DomTree>,
    mssa: Mssa,
    accesses: Accesses,
    updater: MemorySsaUpdater,
}

/// The graph handle of `$state`, over its fields (so the rest stay borrowable).
macro_rules! graph {
    ($state:expr) => {
        Graph {
            mssa: &mut $state.mssa,
            accesses: &mut $state.accesses,
            updater: &mut $state.updater,
            outer: &$state.outer,
            layout: $state.layout,
            last_store: IdMap::new(),
        }
    };
}

impl State<'_> {
    fn query(&self) -> Query<'_> {
        Query {
            context: &*self.context,
            globals: &self.outer.globals,
            metadata: self.metadata,
            function: &*self.function,
            layout: self.layout,
            dt: None,
            assumptions: None,
            dom_conditions: None,
            cond_context: None,
            cxt: None,
        }
    }

    /// The view the alias model reads the function through.
    fn view(&self) -> View<'_> {
        View::within(&*self.context, self.layout, &*self.function, &self.outer)
            .with_registers(&self.registers)
            .with_shape(&self.shape)
    }

    /// `width` bytes at `pointer`: the pointer alone, as `MemoryLocation(Ptr,
    /// Size)`.
    fn reference_at(
        &self,
        unit: &View,
        pointer: Operand,
        width: u32,
    ) -> MemRef {
        MemRef::at(unit, pointer, width)
    }

    /// The memory intrinsic `inst` is, if it is one.
    fn mem(
        &self,
        inst: InstId,
    ) -> Option<Mem> {
        let instruction = self.function.instruction(inst);
        let Opcode::Call(_) = instruction.opcode else { return None };
        let callee = llrm_mir::memory::callee(self.context, self.function, inst)?;
        let global = self.outer.globals.get(callee.0 as usize)?;
        let name = global.name.as_deref()?;
        let kind = match global.intrinsic()? {
            Intrinsic::MemSet => Kind3::Set,
            Intrinsic::MemCpy => Kind3::Copy,
            Intrinsic::MemMove => Kind3::Move,
            _ => return None,
        };
        let operands = &instruction.operands;
        if operands.len() != 5 {
            return None;
        }
        let volatile = self.query().int_constant(operands[3]).is_some_and(|(bits, _)| bits != 0);
        Some(Mem {
            inst,
            kind,
            dest: operands[0],
            other: operands[1],
            length: operands[2],
            volatile,
            force_inlined: name.starts_with("llvm.memcpy.inline"),
        })
    }

    /// `ConstantInt` length of a memory intrinsic.
    fn constant_length(
        &self,
        length: Operand,
    ) -> Option<u64> {
        llrm_mir::intrinsics::length(self.context, length)
    }

    // ---- locations ---------------------------------------------------------

    /// `MemoryLocation::getForDest(M)`: the bytes a memory intrinsic writes.
    fn dest_location(
        &self,
        aa: &AliasQuery,
        mem: &Mem,
    ) -> Option<Rc<Location>> {
        let width = self.constant_length(mem.length).and_then(|bytes| u32::try_from(bytes).ok());
        let reference = self.reference_at(aa.unit(), mem.dest, width.unwrap_or(1));
        Some(aa.location_for(&reference, width.is_none()))
    }

    /// `MemoryLocation::getForSource(M)`: the bytes a transfer reads.
    fn source_location(
        &self,
        aa: &AliasQuery,
        mem: &Mem,
    ) -> Option<Rc<Location>> {
        let width = self.constant_length(mem.length).and_then(|bytes| u32::try_from(bytes).ok());
        let reference = self.reference_at(aa.unit(), mem.other, width.unwrap_or(1));
        Some(aa.location_for(&reference, width.is_none()))
    }

    // ---- the graph ---------------------------------------------------------

    /// `eraseInstruction`: the access goes, then the instruction.
    fn erase_instruction(
        &mut self,
        inst: InstId,
    ) {
        graph!(self).erase(&*self.function, inst);
        self.function.erase(inst).unwrap_or_else(|error| panic!("memcpyopt: {error}"));
    }

    /// `createMemoryAccessAfter` / `Before` and `insertDef`.
    fn insert_def(
        &mut self,
        inst: InstId,
        at: Created,
        rename_uses: bool,
    ) {
        let block = self.function.parent(inst).expect("a placed instruction");
        graph!(self).created_renaming(&*self.context, &*self.function, inst, block, at, rename_uses);
    }

    // ---- the checks on the graph -------------------------------------------

    /// `accessedBetween`: whether any instruction strictly between `start` and
    /// `end`, in one block, may read or write `loc`. The first `lifetime.start`
    /// that does goes to `skipped` and is not counted.
    fn accessed_between(
        &self,
        aa: &AliasQuery,
        loc: &Location,
        start: AccessId,
        end: AccessId,
        mut skipped: Option<&mut Option<InstId>>,
    ) -> bool {
        let block = self.mssa.access(start).block;
        debug_assert_eq!(block, self.mssa.access(end).block, "Only local supported");
        let list = self.mssa.get_block_accesses(block).expect("a placed access");
        let from = list.iter().position(|&one| one == start).expect("a placed access") + 1;
        let to = list.iter().position(|&one| one == end).expect("a placed access");
        for &access in &list[from..to] {
            let inst = self.mssa.access(access).inst.expect("a def or use has an instruction");
            if !aa.get_mod_ref_info(inst, loc).is_mod_or_ref() {
                continue;
            }
            if self.intrinsic_of(inst) == Some(Intrinsic::LifetimeStart)
                && let Some(skipped) = skipped.as_deref_mut()
                && skipped.is_none()
            {
                *skipped = Some(inst);
                continue;
            }
            return true;
        }
        false
    }

    // ---- the pass ----------------------------------------------------------

    /// `runImpl`: the function walked until a walk changes nothing.
    fn run_impl(&mut self) -> bool {
        let mut made_change = false;
        while self.iterate_on_function() {
            made_change = true;
        }
        made_change
    }

    /// `iterateOnFunction`.
    fn iterate_on_function(&mut self) -> bool {
        let mut made_change = false;
        for block in self.function.layout().to_vec() {
            if !self.dt.is_reachable_from_entry(block) {
                continue;
            }
            let mut bi = self.function.block(block).first();
            while let Some(inst) = bi {
                bi = self.function.next_in_block(inst);
                let mut repeat_instruction = false;
                if let Some(mem) = self.mem(inst) {
                    match mem.kind {
                        // `processMemSet` is stage 1's.
                        Kind3::Set => {}
                        Kind3::Copy => repeat_instruction = self.process_memcpy(mem, &mut bi),
                        Kind3::Move => repeat_instruction = self.process_memmove(mem, &mut bi),
                    }
                } else if matches!(self.function.instruction(inst).opcode, Opcode::Store { .. }) {
                    repeat_instruction = self.process_store(inst, &mut bi);
                } else if matches!(self.function.instruction(inst).opcode, Opcode::Call(_)) {
                    for index in 0..self.function.instruction(inst).operands.len().saturating_sub(1) {
                        let attrs = self.argument_attrs(inst, index);
                        let by_value = attrs
                            .iter()
                            .any(
                                |attr| matches!(
                                    attr,
                                    llrm_mir::opcode::Attribute::Type(name, _) if name == "byval"
                                ),
                            );
                        let facts = llrm_mir::facts::Facts::of(&attrs);
                        if by_value {
                            made_change |= self.process_byval_argument(inst, index);
                        } else if facts.read_only() || facts.read_none() {
                            made_change |= self.process_immut_argument(inst, index);
                        }
                    }
                }
                if repeat_instruction {
                    // `--BI` when not at the beginning.
                    bi = match bi {
                        Some(next) => self.function.prev_in_block(next).or(Some(next)),
                        None => self.function.block(block).last(),
                    };
                    made_change = true;
                }
            }
        }
        made_change
    }

    /// `isZeroSize`.
    fn is_zero_size(
        &self,
        size: Operand,
    ) -> bool {
        let size = match size {
            Operand::Value(value) => match self.function.value(value).def {
                llrm_mir::module::ValueDef::Instruction(inst) => {
                    llrm_mir::simplify::simplify_instruction(self.context, self.layout, self.function, inst)
                        .unwrap_or(size)
                }
                _ => size,
            },
            other => other,
        };
        let Operand::Constant(id) = size else { return false };
        matches!(
            self.context.get(id).kind,
            ConstantKind::Undef | ConstantKind::Poison | ConstantKind::Zero
        )
            || matches!(self.context.get(id).kind, ConstantKind::Int(0))
    }

    /// `hasUndefContents`: whether `def` leaves the bytes of `v` undefined.
    fn has_undef_contents(
        &self,
        v: Operand,
        def: AccessId,
    ) -> bool {
        let q = self.query();
        let object = get_underlying_object(&q, v, 0);
        let is_alloca = matches!(
            object,
            Operand::Value(value) if matches!(
                self.function.value(value).def,
                llrm_mir::module::ValueDef::Instruction(inst)
                    if matches!(self.function.instruction(inst).opcode, Opcode::Alloca { .. })
            )
        );
        if self.mssa.is_live_on_entry_def(def) {
            return is_alloca;
        }
        if let Some(inst) = self.mssa.access(def).inst
            && let Some(Intrinsic::LifetimeStart) = self.intrinsic_of(inst)
            && is_alloca
        {
            return self.function.instruction(inst).operands[1] == object
                || self.function.instruction(inst).operands[0] == object;
        }
        false
    }

    /// The intrinsic `inst` calls.
    fn intrinsic_of(
        &self,
        inst: InstId,
    ) -> Option<Intrinsic> {
        let callee = llrm_mir::memory::callee(self.context, self.function, inst)?;
        self.outer.globals.get(callee.0 as usize)?.intrinsic()
    }

    /// `overreadUndefContents`: whether the bytes `memcpy` reads past what
    /// `mem_src` wrote are undefined.
    fn overread_undef_contents(
        &self,
        aa: &AliasQuery,
        memcpy: &Mem,
        mem_src: &Mem,
    ) -> bool {
        let Some(loc) = self.source_location(aa, memcpy) else { return false };
        let Some(src_access) = self.mssa.get_memory_access(mem_src.inst) else { return false };
        let defining = self.mssa.access(src_access).defining.expect("a defining access");
        let mut limit = llrm_analysis::mssa::walker::MAX_CHECK_LIMIT;
        let clobber = self.mssa.get_clobbering_memory_access_for_location(aa, defining, &loc, &mut limit);
        self.mssa.access(clobber).kind == Kind::Def && self.has_undef_contents(memcpy.other, clobber)
    }

    /// `processMemCpy`.
    fn process_memcpy(
        &mut self,
        m: Mem,
        bbi: &mut Option<InstId>,
    ) -> bool {
        // We can only optimize non-volatile memcpy's.
        if m.volatile {
            return false;
        }
        // If the source and destination of the memcpy are the same, then zap
        // it.
        if m.other == m.dest {
            *bbi = bbi.and_then(|next| self.function.next_in_block(next));
            self.erase_instruction(m.inst);
            return true;
        }
        // If the size is zero, remove the memcpy.
        if self.is_zero_size(m.length) {
            *bbi = bbi.and_then(|next| self.function.next_in_block(next));
            self.erase_instruction(m.inst);
            return true;
        }
        let Some(ma) = self.mssa.get_memory_access(m.inst) else { return false };

        // If copying from a constant, try to turn the memcpy into a memset.
        let source_object = get_underlying_object(&self.query(), m.other, 0);
        if let Operand::Constant(id) = source_object
            && let ConstantKind::Global(global) = self.context.get(id).kind
            && let Some(GlobalKind::Variable(variable)) = self.outer.globals.get(global.0 as usize).map(|g| &g.kind)
            && variable.constant
            && let Some(initializer) = variable.initializer
            && let Some(byte) = is_bytewise_value(&self.query(), Operand::Constant(initializer))
        {
            let value = self.byte_operand(byte);
            let align = self.align_of(m.inst, 0);
            let new_m = create_mem_set(
                &mut *self.context,
                &mut *self.declared,
                &mut *self.function,
                m.inst,
                m.dest,
                value,
                m.length,
                align,
            );
            // The memset takes the memcpy's place in the graph.
            self.insert_def(new_m, Created::After(ma), true);
            self.erase_instruction(m.inst);
            return true;
        }

        let any_clobber = self.mssa.access(ma).defining.expect("a defining access");
        let (dest_clobber, source_clobber) = {
            let view = self.view();
            let aa = AliasQuery::new(&view, &self.accesses);
            let mut limit = llrm_analysis::mssa::walker::MAX_CHECK_LIMIT;
            let dest = self
                .dest_location(&aa, &m)
                .map(|loc| self.mssa.get_clobbering_memory_access_for_location(&aa, any_clobber, &loc, &mut limit));
            let source = self
                .source_location(&aa, &m)
                .map(|loc| self.mssa.get_clobbering_memory_access_for_location(&aa, any_clobber, &loc, &mut limit));
            (dest, source)
        };
        if let Some(clobber) = dest_clobber
            && self.mssa.access(clobber).kind == Kind::Def
            && let Some(inst) = self.mssa.access(clobber).inst
            && let Some(memset) = self.mem(inst).filter(|one| one.kind == Kind3::Set)
            && self.mssa.access(clobber).block == self.function.parent(m.inst).expect("a placed memcpy")
            && self.process_memset_memcpy_dependence(&m, &memset)
        {
            return true;
        }
        if let Some(clobber) = source_clobber
            && self.mssa.access(clobber).kind == Kind::Def
        {
            if let Some(inst) = self.mssa.access(clobber).inst {
                if matches!(self.function.instruction(inst).opcode, Opcode::Call(_))
                    && let Some(size) = self.constant_length(m.length)
                    && self.perform_call_slot_optzn(
                        m.inst,
                        m.inst,
                        m.dest,
                        m.other,
                        size,
                        self.align_of(m.inst, 0).unwrap_or(1),
                        |_| Some(inst),
                    )
                {
                    self.erase_instruction(m.inst);
                    return true;
                }
                if let Some(dependence) = self.mem(inst) {
                    if dependence.kind == Kind3::Copy && self.process_memcpy_memcpy_dependence(&m, &dependence) {
                        return true;
                    }
                    if dependence.kind == Kind3::Set && self.perform_memcpy_to_memset_optzn(&m, &dependence) {
                        // Converted memcpy to memset.
                        self.erase_instruction(m.inst);
                        return true;
                    }
                }
            }
            if self.has_undef_contents(m.other, clobber) {
                // Removed memcpy from undef.
                self.erase_instruction(m.inst);
                return true;
            }
        }
        // A copy of one stack slot to another may merge the two.
        if let Some(size) = self.constant_length(m.length)
            && self.perform_stack_move_optzn(m.inst, m.inst, m.dest, m.other, size)
        {
            // Do not invalidate the iterator.
            *bbi = self.function.next_in_block(m.inst);
            self.erase_instruction(m.inst);
            return true;
        }
        false
    }

    /// `isMustAlias(V1, V2)`: the same address, as one byte each.
    fn must_alias(
        &self,
        aa: &AliasQuery,
        one: Operand,
        other: Operand,
    ) -> bool {
        let unit = aa.unit();
        let (a, b) = (
            aa.location_for(&self.reference_at(unit, one, 1), false),
            aa.location_for(&self.reference_at(unit, other, 1), false),
        );
        aa.alias(&a, &b) == llrm_analysis::mssa::alias::AliasResult::Must
    }

    /// `processMemSetMemCpyDependence`: `memcpy` follows a `memset` of the same
    /// destination, and copies its tail over: the memset only needs to write
    /// what the copy leaves.
    fn process_memset_memcpy_dependence(
        &mut self,
        memcpy: &Mem,
        memset: &Mem,
    ) -> bool {
        let (Some(memcpy_access), Some(memset_access)) =
            (self.mssa.get_memory_access(memcpy.inst), self.mssa.get_memory_access(memset.inst))
        else {
            return false;
        };
        {
            let view = self.view();
            let aa = AliasQuery::new(&view, &self.accesses);
            // We can only transform memset/memcpy with the same destination.
            if !self.must_alias(&aa, memset.dest, memcpy.dest) {
                return false;
            }
            // Don't perform the transform if src_size may be zero: the memset
            // would have not been a dead store.
            if memset.volatile || !is_known_non_zero(&self.query(), memcpy.length, 0) {
                return false;
            }
            // Check that src and dst of the memcpy aren't the same. While
            // memcpy requires them to not overlap, if they do, the memcpy is
            // still ok to delete.
            let Some(source) = self.source_location(&aa, memcpy) else { return false };
            if aa.get_mod_ref_info(memcpy.inst, &source).is_mod() {
                return false;
            }
            // Check that nothing touches the dest of the memset between the
            // memset and the memcpy.
            let Some(dest) = self.dest_location(&aa, memset) else { return false };
            if self.accessed_between(&aa, &dest, memset_access, memcpy_access, None) {
                return false;
            }
        }
        // (No unwinding in MIR: `mayBeVisibleThroughUnwinding` is false.)
        let (dest_size, src_size) = (memset.length, memcpy.length);
        if dest_size == src_size {
            // The memset is dead: the memcpy overwrites all of it.
            self.erase_instruction(memset.inst);
            return true;
        }
        let dest_align = self.align_of(memset.inst, 0).unwrap_or(1).max(self.align_of(memcpy.inst, 0).unwrap_or(1));
        let mut alignment = 1;
        if dest_align > 1
            && let Some(copy_bytes) = self.constant_length(src_size)
        {
            alignment = common_alignment(dest_align, copy_bytes);
        }
        let dest = memcpy.dest;
        let (dest_size, src_size) = self.same_width(memcpy.inst, dest_size, src_size);
        // Both known: the builder folds the arithmetic.
        let (memset_len, tail) = match (self.constant_length_of(dest_size), self.constant_length_of(src_size)) {
            (Some(dest_bytes), Some(src_bytes)) => {
                let length = if dest_bytes <= src_bytes { 0 } else { dest_bytes - src_bytes };
                let ty = self.function.operand_type(self.context, dest_size).expect("a typed length");
                (Operand::Constant(self.context.int(ty, i128::from(length))), src_size)
            }
            _ => {
                let ty = self.function.operand_type(self.context, dest_size).expect("a typed length");
                let i1 = self.context.types.int(1);
                let ule = self.insert_before(
                    memcpy.inst,
                    Opcode::ICmp(IntPredicate::Ule),
                    i1,
                    vec![dest_size, src_size],
                    Flags::default(),
                );
                let diff = self.insert_before(
                    memcpy.inst,
                    Opcode::Binary(BinaryOp::Sub),
                    ty,
                    vec![dest_size, src_size],
                    Flags::default(),
                );
                let zero = Operand::Constant(self.context.int(ty, 0));
                let select =
                    self.insert_before(memcpy.inst, Opcode::Select, ty, vec![ule, zero, diff], Flags::default());
                (select, src_size)
            }
        };
        // `Builder.CreatePtrAdd(Dest, SrcSize)`.
        let i8 = self.context.types.int(8);
        let ptr_ty = self.function.operand_type(self.context, dest).expect("a typed pointer");
        let tail_ptr = self.insert_before(
            memcpy.inst,
            Opcode::GetElementPtr { source: i8 },
            ptr_ty,
            vec![dest, tail],
            Flags::default(),
        );
        let new_memset = create_mem_set(
            &mut *self.context,
            &mut *self.declared,
            &mut *self.function,
            memcpy.inst,
            tail_ptr,
            memset.other,
            memset_len,
            Some(alignment),
        );
        self.insert_def(new_memset, Created::Before(memcpy_access), true);
        self.erase_instruction(memset.inst);
        true
    }

    /// `ConstantInt` value of an operand.
    fn constant_length_of(
        &self,
        length: Operand,
    ) -> Option<u64> {
        llrm_mir::intrinsics::length(self.context, length)
    }

    /// The two sizes at the wider of their widths, constants widened.
    fn same_width(
        &mut self,
        before: InstId,
        dest_size: Operand,
        src_size: Operand,
    ) -> (Operand, Operand) {
        let width = |this: &Self, op: Operand| {
            this.function.operand_type(this.context, op).and_then(|ty| this.context.types.int_bits(ty)).unwrap_or(0)
        };
        let (dest_bits, src_bits) = (width(self, dest_size), width(self, src_size));
        if dest_bits == src_bits {
            return (dest_size, src_size);
        }
        let to = dest_bits.max(src_bits);
        let widen = |this: &mut Self, op: Operand, bits: u32| -> Operand {
            if bits == to {
                return op;
            }
            let ty = this.context.types.int(to);
            if let Some(bytes) = this.constant_length_of(op) {
                return Operand::Constant(this.context.int(ty, i128::from(bytes)));
            }
            this.insert_before(before, Opcode::Cast(CastOp::ZExt), ty, vec![op], Flags::default())
        };
        (widen(self, dest_size, dest_bits), widen(self, src_size, src_bits))
    }

    /// An instruction built before `at`, as the builder does.
    fn insert_before(
        &mut self,
        at: InstId,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
        flags: Flags,
    ) -> Operand {
        let new = self.function.create_instruction(opcode, ty, operands, flags, None);
        self.function.insert(new, llrm_mir::edit::Position::Before(at)).expect("a placed instruction");
        Operand::Value(self.function.instruction(new).result.expect("an instruction with a value"))
    }

    /// `processMemCpyMemCpyDependence`: `m` copies what `mdep` copied: copy
    /// from `mdep`'s source instead.
    fn process_memcpy_memcpy_dependence(
        &mut self,
        m: &Mem,
        mdep: &Mem,
    ) -> bool {
        // We can only optimize non-volatile memcpy's.
        if mdep.volatile {
            return false;
        }
        let (Some(m_access), Some(mdep_access)) =
            (self.mssa.get_memory_access(m.inst), self.mssa.get_memory_access(mdep.inst))
        else {
            return false;
        };
        // What is copied: from `forward` bytes into what `mdep` wrote, `length`
        // of them.
        let Some((forward, copy_length)) = ({
            let view = self.view();
            let aa = AliasQuery::new(&view, &self.accesses);
            self.forwarded_range(&aa, m, mdep)
        }) else {
            return false;
        };

        // The source the new copy reads: `mdep`'s, past `forward` bytes. The
        // address is made before the checks and goes again if one fails
        // (`NewCopySource` and its cleanup).
        let mut copy_source = mdep.other;
        let mut copy_source_align = self.align_of(mdep.inst, 1);
        let mut new_copy_source: Option<InstId> = None;
        if forward > 0 {
            let m_dest_offset = get_pointer_offset_from(&self.query(), m.dest, mdep.other);
            if m_dest_offset == Some(forward) {
                copy_source = m.dest;
            } else {
                let i8 = self.context.types.int(8);
                let i64_ty = self.context.types.int(64);
                let ptr_ty = self.function.operand_type(self.context, mdep.other).expect("a typed pointer");
                let offset = Operand::Constant(self.context.int(i64_ty, i128::from(forward)));
                let made = self
                    .function
                    .create_instruction(
                        Opcode::GetElementPtr { source: i8 },
                        ptr_ty,
                        vec![mdep.other, offset],
                        Flags::INBOUNDS,
                        None,
                    );
                self.function.insert(made, llrm_mir::edit::Position::Before(m.inst)).expect("a placed instruction");
                new_copy_source = Some(made);
                copy_source = Operand::Value(self.function.instruction(made).result.expect("an address"));
            }
            copy_source_align = copy_source_align.map(|align| common_alignment(align, forward as u64));
        }
        let cleanup = |this: &mut Self, made: Option<InstId>| {
            if let Some(made) = made {
                let result = this.function.instruction(made).result.expect("an address");
                if this.function.users(result).is_empty() {
                    this.function.erase(made).expect("an unused address");
                }
            }
        };
        let (written, must, modified) = {
            let view = self.view();
            let aa = AliasQuery::new(&view, &self.accesses);
            let unit = aa.unit();
            // The bytes read of MDep: its source, `forward` on, `copy_length`
            // long.
            let bytes = if copy_length == mdep.length {
                self.constant_length(mdep.length).and_then(|bytes| u32::try_from(bytes).ok())
            } else {
                self.constant_length(copy_length).and_then(|bytes| u32::try_from(bytes).ok())
            };
            let reference = self.reference_at(unit, copy_source, bytes.unwrap_or(1));
            let loc = aa.location_for(&reference, bytes.is_none());
            let written = self.written_between_shared(&aa, &loc, mdep_access, m_access);
            let must = self.must_alias(&aa, m.dest, copy_source);
            let modified =
                self.source_location(&aa, mdep).is_some_and(|source| aa.get_mod_ref_info(m.inst, &source).is_mod());
            (written, must, modified)
        };
        if written {
            cleanup(self, new_copy_source);
            return false;
        }
        // If the dest of the second might alias the source of the first, then
        // the source and dest might overlap. In addition, if the source of the
        // first points to constant memory, they won't overlap by definition.
        // Otherwise, we still do the transform, using a memmove.
        if must {
            self.erase_instruction(m.inst);
            cleanup(self, new_copy_source);
            return true;
        }
        let use_memmove = modified;
        if use_memmove && m.force_inlined {
            cleanup(self, new_copy_source);
            return false;
        }
        // Forced-inline copies are not made again (DEVIATE 5).
        if m.force_inlined {
            cleanup(self, new_copy_source);
            return false;
        }
        let how = if use_memmove { Copy::Memmove } else { Copy::Memcpy };
        let dest_align = self.align_of(m.inst, 0);
        let new_m = create_mem_copy(
            &mut *self.context,
            &mut *self.declared,
            &mut *self.function,
            m.inst,
            how,
            m.dest,
            dest_align,
            copy_source,
            copy_source_align,
            copy_length,
        );
        self.set_volatile(new_m, m.volatile);
        self.insert_def(new_m, Created::After(m_access), true);
        self.erase_instruction(m.inst);
        true
    }

    /// The first half of `processMemCpyMemCpyDependence`: where in what `mdep`
    /// wrote `m` copies from, and how much of it is there to copy.
    fn forwarded_range(
        &self,
        aa: &AliasQuery,
        m: &Mem,
        mdep: &Mem,
    ) -> Option<(i64, Operand)> {
        // If dep instruction is reading from our current input, then it is a
        // noop transfer and substituting the input won't change this
        // instruction. Just ignore the input and let someone else zap MDep.
        if self.must_alias(aa, mdep.dest, mdep.other) {
            return None;
        }
        let mut forward = 0_i64;
        if m.other != mdep.dest {
            let offset = get_pointer_offset_from(&self.query(), m.other, mdep.dest)?;
            if offset < 0 {
                return None;
            }
            forward = offset;
        }
        let mut copy_length = m.length;
        if forward != 0 || mdep.length != copy_length {
            // The lengths must be constant to be compared.
            let mdep_len = self.constant_length(mdep.length)?;
            let m_len = self.constant_length(copy_length)?;
            if mdep_len < m_len + forward as u64 {
                // The copy reads past what MDep wrote: all right only where
                // the extra bytes are undefined anyway.
                if !self.overread_undef_contents(aa, m, mdep) {
                    return None;
                }
                if mdep_len <= forward as u64 {
                    return None; // Should not reach here (no aliasing).
                }
                let ty = self.function.operand_type(self.context, copy_length)?;
                copy_length = Operand::Constant(self.context.int(ty, i128::from(mdep_len - forward as u64)));
            }
        }
        Some((forward, copy_length))
    }

    /// `performMemCpyToMemSetOptzn`: `memcpy` reads bytes a `memset` wrote:
    /// it becomes a memset of what it copies.
    fn perform_memcpy_to_memset_optzn(
        &mut self,
        memcpy: &Mem,
        memset: &Mem,
    ) -> bool {
        let memset_size = memset.length;
        let mut copy_size = memcpy.length;
        let mut m_offset = 0_i64;
        if memcpy.other != memset.dest {
            // Without a constant offset, don't know how the memcpy relates.
            let Some(offset) = get_pointer_offset_from(&self.query(), memcpy.other, memset.dest) else { return false };
            m_offset = offset;
        }
        if m_offset != 0 || memset_size != copy_size {
            let (c_memset, c_copy) = (self.constant_length(memset_size), self.constant_length(copy_size));
            let covered = matches!(
                (c_memset, c_copy),
                (Some(set), Some(copy)) if m_offset >= 0 && copy as i64 + m_offset <= set as i64
            );
            if !covered {
                // The memcpy reads past what was set: all right where the
                // memset's own bytes there are undefined.
                let ok = {
                    let view = self.view();
                    let aa = AliasQuery::new(&view, &self.accesses);
                    self.overread_undef_contents(&aa, memcpy, memset)
                };
                if !ok {
                    return false;
                }
                if let (Some(set), Some(copy)) = (c_memset, c_copy) {
                    let new_size = if m_offset < 0 {
                        let offset = (-m_offset) as u64;
                        copy.saturating_sub(offset)
                    } else if m_offset == 0 {
                        set
                    } else {
                        set.saturating_sub(m_offset as u64)
                    };
                    let ty = self.function.operand_type(self.context, copy_size).expect("a typed length");
                    copy_size = Operand::Constant(self.context.int(ty, i128::from(new_size)));
                } else if m_offset < 0 {
                    return false;
                }
            }
        }
        let Some(memcpy_access) = self.mssa.get_memory_access(memcpy.inst) else { return false };
        let mut dest_ptr = memcpy.dest;
        let mut align = self.align_of(memcpy.inst, 0);
        if m_offset < 0 {
            let i8 = self.context.types.int(8);
            let i64_ty = self.context.types.int(64);
            let ptr_ty = self.function.operand_type(self.context, dest_ptr).expect("a typed pointer");
            let offset = Operand::Constant(self.context.int(i64_ty, i128::from(-m_offset)));
            dest_ptr = self.insert_before(
                memcpy.inst,
                Opcode::GetElementPtr { source: i8 },
                ptr_ty,
                vec![dest_ptr, offset],
                Flags::default(),
            );
            align = align.map(|align| common_alignment(align, (-m_offset) as u64));
        }
        let new_m = create_mem_set(
            &mut *self.context,
            &mut *self.declared,
            &mut *self.function,
            memcpy.inst,
            dest_ptr,
            memset.other,
            copy_size,
            align,
        );
        self.insert_def(new_m, Created::After(memcpy_access), true);
        true
    }

    /// `processMemMove`.
    fn process_memmove(
        &mut self,
        m: Mem,
        bbi: &mut Option<InstId>,
    ) -> bool {
        let overlaps = {
            let view = self.view();
            let aa = AliasQuery::new(&view, &self.accesses);
            self.source_location(&aa, &m).is_none_or(|source| aa.get_mod_ref_info(m.inst, &source).is_mod())
        };
        if overlaps {
            if !m.volatile && self.is_memmove_memset_dependency(&m) {
                // Removed redundant memmove.
                *bbi = bbi.and_then(|next| self.function.next_in_block(next));
                self.erase_instruction(m.inst);
                return true;
            }
            return false;
        }
        // Optimizing memmove -> memcpy: the call names the memcpy intrinsic.
        let Opcode::Call(info) = self.function.instruction(m.inst).opcode.clone() else { return false };
        let Some(callee) = llrm_mir::memory::callee(self.context, self.function, m.inst) else { return false };
        let name = self.outer.globals[callee.0 as usize].name.owned().unwrap_or_default();
        let renamed = name.replacen("llvm.memmove.", "llvm.memcpy.", 1);
        let function_type = info.function_type;
        let id = self.declared.declare(&renamed, function_type);
        let ptr = self.context.types.ptr(0);
        let callee = Operand::Constant(self.context.constant(Constant { ty: ptr, kind: ConstantKind::Global(id) }));
        let last = self.function.instruction(m.inst).operands.len() - 1;
        self.function.set_operand(m.inst, last, callee);
        true
    }

    /// `stripPointerCasts`: casts and all-zero GEPs looked through.
    fn strip_pointer_casts(
        &self,
        mut operand: Operand,
    ) -> Operand {
        loop {
            let Operand::Value(value) = operand else { return operand };
            let llrm_mir::module::ValueDef::Instruction(def) = self.function.value(value).def else { return operand };
            let instruction = self.function.instruction(def);
            match instruction.opcode {
                Opcode::Cast(CastOp::BitCast) => operand = instruction.operands[0],
                // A selector cast back to far is `segment:0`, not the pointer
                // it was made from (DEVIATE C1 of BasicAA).
                Opcode::Cast(CastOp::AddrSpaceCast)
                    if llrm_analysis::basicaa::BasicAA::new(&self.view()).passes_through(instruction) =>
                {
                    operand = instruction.operands[0]
                }
                Opcode::GetElementPtr { .. }
                    if instruction.operands[1..]
                        .iter()
                        .all(|&one| self.query().int_constant(one).is_some_and(|(bits, _)| bits == 0)) =>
                {
                    operand = instruction.operands[0]
                }
                _ => return operand,
            }
        }
    }

    /// `isDereferenceablePointer(ptr, size)`: the bytes known at `ptr`, or at
    /// the base a constant GEP reaches it from, hold `size` bytes.
    fn is_dereferenceable(
        &self,
        ptr: Operand,
        size: u64,
    ) -> bool {
        let q = self.query();
        let (mut ptr, mut offset) = (ptr, 0i128);
        loop {
            let (bytes, _) = llrm_mir::valuetracking::known::pointer_dereferenceable_bytes(&q, ptr);
            if offset >= 0 && i128::from(bytes) >= offset + i128::from(size) {
                return true;
            }
            let Operand::Value(value) = ptr else { return false };
            let llrm_mir::module::ValueDef::Instruction(def) = self.function.value(value).def else { return false };
            let instruction = self.function.instruction(def);
            let Opcode::GetElementPtr { source } = instruction.opcode else { return false };
            let indices: Vec<Option<i128>> = instruction.operands[1..]
                .iter()
                .map(|&one| q.int_constant(one).map(|(bits, width)| llrm_mir::context::signed(bits, width)))
                .collect();
            let (constant, variable) = self.layout.collect_offset(&self.context.types, source, &indices);
            if !variable.is_empty() {
                return false;
            }
            offset += constant;
            ptr = instruction.operands[0];
        }
    }

    /// Whether `def` is available at `user`: `DT->dominates(Value,
    /// Instruction)`.
    fn value_dominates(
        &self,
        def: Operand,
        user: InstId,
    ) -> bool {
        match def {
            Operand::Value(value) => match self.function.value(value).def {
                llrm_mir::module::ValueDef::Instruction(inst) => {
                    self.dt.instruction_dominates(self.function, inst, user)
                }
                llrm_mir::module::ValueDef::Argument(_) => true,
            },
            _ => true,
        }
    }

    /// `performCallSlotOptzn`: `call f(.., src, ..)` followed by a copy of the
    /// alloca `src` to `cpy_dest` calls with `cpy_dest` instead, and the copy
    /// goes (the caller erases it). `cpy_load` and `cpy_store` are the copy:
    /// one memcpy, or a load and its store. `get_c` finds the call lazily.
    fn perform_call_slot_optzn(
        &mut self,
        cpy_load: InstId,
        cpy_store: InstId,
        cpy_dest: Operand,
        cpy_src: Operand,
        cpy_size: u64,
        cpy_dest_align: u64,
        get_c: impl FnOnce(&Self) -> Option<InstId>,
    ) -> bool {
        // Require that src be an alloca, of a size known.
        let Operand::Value(src_value) = cpy_src else { return false };
        let llrm_mir::module::ValueDef::Instruction(src_alloca) = self.function.value(src_value).def else {
            return false;
        };
        let Opcode::Alloca { allocated, align, .. } = self.function.instruction(src_alloca).opcode else {
            return false;
        };
        let count = match self.function.instruction(src_alloca).operands.first() {
            None => 1,
            Some(&count) => match self.query().int_constant(count) {
                Some((bits, _)) => bits as u64,
                None => return false,
            },
        };
        let src_size = self.layout.alloc_size(&self.context.types, allocated) * count;
        if cpy_size < src_size {
            return false;
        }
        let Some(c) = get_c(&*self) else { return false };
        let Opcode::Call(info) = &self.function.instruction(c).opcode else { return false };
        let info = (**info).clone();
        // Lifetime marks shouldn't be operated on.
        if self.intrinsic_of(c) == Some(Intrinsic::LifetimeStart) {
            return false;
        }
        if self.function.parent(c) != self.function.parent(cpy_store) {
            return false;
        }
        let call_args = self.function.instruction(c).operands.len() - 1;
        let src_align = align.unwrap_or_else(|| self.layout.align(&self.context.types, allocated));
        let dest_object = get_underlying_object(&self.query(), cpy_dest, 0);

        let mut skipped_lifetime_start = None;
        let mut move_gep = false;
        {
            let view = self.view();
            let aa = AliasQuery::new(&view, &self.accesses);
            // Check that nothing touches the dest of the copy between the call
            // and the store/memcpy.
            let dest_loc = if matches!(
                self.function.instruction(cpy_store).opcode,
                Opcode::Store { .. }
            ) {
                self.access_location(&aa, cpy_store)
            } else {
                self.mem(cpy_store).and_then(|m| self.dest_location(&aa, &m))
            };
            let Some(dest_loc) = dest_loc else { return false };
            let (Some(c_access), Some(store_access)) =
                (self.mssa.get_memory_access(c), self.mssa.get_memory_access(cpy_store))
            else {
                return false;
            };
            if self.accessed_between(&aa, &dest_loc, c_access, store_access, Some(&mut skipped_lifetime_start)) {
                return false;
            }
            // A lifetime.start moved above the call must be movable: its
            // argument defined after the call would have to move too.
            if let Some(start) = skipped_lifetime_start
                && let Operand::Value(value) = self.function.instruction(start).operands[1]
                && let llrm_mir::module::ValueDef::Instruction(def) = self.function.value(value).def
                && self.function.parent(def) == self.function.parent(c)
                && self.function.comes_before(c, def)
            {
                return false;
            }
            // Storing to the first src_size bytes of dest must not trap.
            let writable =
                crate::licm::promote::is_writable_object(self.context, self.function, &self.outer, dest_object);
            if (!writable || !self.is_dereferenceable(cpy_dest, cpy_size))
                && !llrm_mir::valuetracking::guarantee::is_guaranteed_to_transfer_execution_to_successor_range(
                    &self.query(),
                    std::iter::successors(Some(c), |&inst| self.function.next_in_block(inst))
                        .take_while(|&inst| inst != cpy_store),
                    32,
                )
            {
                return false;
            }
            // `mayBeVisibleThroughUnwinding` is false: nothing unwinds (DEVIATE
            // 2).
            let dest_is_alloca = crate::licm::promote::is_alloca_in(self.function, cpy_dest);
            let dest_sufficiently_aligned = src_align <= cpy_dest_align;
            if !dest_sufficiently_aligned && !dest_is_alloca {
                return false;
            }
            // src is not accessed except by the call and the copy: it holds
            // undefined values when passed in, so the copy can be dropped.
            let mut uses = vec![src_value];
            while let Some(value) = uses.pop() {
                for &used in self.function.users(value) {
                    let user = used.user;
                    match self.function.instruction(user).opcode {
                        Opcode::Cast(CastOp::AddrSpaceCast) => {
                            if let Some(result) = self.function.instruction(user).result {
                                uses.push(result);
                            }
                            continue;
                        }
                        _ => {}
                    }
                    if matches!(
                        self.intrinsic_of(user),
                        Some(Intrinsic::LifetimeStart | Intrinsic::LifetimeEnd)
                    ) {
                        continue;
                    }
                    if user != c && user != cpy_load {
                        return false;
                    }
                }
            }
            // Whether src is captured by the called function.
            let src_is_captured = (0..call_args).any(|index| {
                self.strip_pointer_casts(self.function.instruction(c).operands[index]) == cpy_src
                    && !info
                        .argument_attrs
                        .get(index)
                        .is_some_and(|attrs| llrm_mir::facts::Facts::of(attrs).no_capture())
            });
            if src_is_captured {
                // dest must not be captured before the call, and src may not be
                // used through the captured pointer before its lifetime ends.
                // DEVIATE 7: `PointerMayBeCapturedBefore` is
                // `PointerMayBeCaptured` anywhere.
                let identified_local = crate::licm::promote::is_alloca_in(self.function, dest_object);
                if !identified_local || self.may_be_captured(dest_object) {
                    return false;
                }
                let src_loc = aa.location_for(&self.reference_at(aa.unit(), cpy_src, src_size as u32), false);
                let mut at = self.function.next_in_block(c);
                while let Some(inst) = at {
                    at = self.function.next_in_block(inst);
                    if self.intrinsic_of(inst) == Some(Intrinsic::LifetimeEnd)
                        && self.function.instruction(inst).operands.get(1) == Some(&cpy_src)
                    {
                        break;
                    }
                    if matches!(self.function.instruction(inst).opcode, Opcode::Ret) {
                        break;
                    }
                    if inst == cpy_load {
                        continue;
                    }
                    if aa.get_mod_ref_info(inst, &src_loc).is_mod_or_ref()
                        || self.function.instruction(inst).opcode.is_terminator()
                    {
                        return false;
                    }
                }
            }
            // The new parameter must dominate the call.
            if !self.value_dominates(cpy_dest, c) {
                // A constant-index GEP can move before the call.
                let Operand::Value(value) = cpy_dest else { return false };
                let llrm_mir::module::ValueDef::Instruction(gep) = self.function.value(value).def else {
                    return false;
                };
                let instruction = self.function.instruction(gep);
                let all_constant =
                    instruction.operands[1..].iter().all(|&one| self.query().int_constant(one).is_some());
                if !(matches!(instruction.opcode, Opcode::GetElementPtr { .. })
                    && all_constant
                    && self.value_dominates(instruction.operands[0], c))
                {
                    return false;
                }
                move_gep = true;
            }
            // The call may not reach dest by itself. DEVIATE 8: a mod/ref the
            // alias model reports ends the attempt (`callCapturesBefore` is not
            // modelled).
            let dest_with_src_size = aa.location_for(&self.reference_at(aa.unit(), cpy_dest, src_size as u32), false);
            if aa.get_mod_ref_info(c, &dest_with_src_size).is_mod_or_ref() {
                return false;
            }
            // No address space casts are made here.
            let (src_ty, dest_ty) =
                (self.function.operand_type(self.context, cpy_src), self.function.operand_type(self.context, cpy_dest));
            if src_ty != dest_ty {
                return false;
            }
            for index in 0..call_args {
                let argument = self.function.instruction(c).operands[index];
                if self.strip_pointer_casts(argument) == cpy_src
                    && self.function.operand_type(self.context, argument) != src_ty
                {
                    return false;
                }
            }
            if !dest_sufficiently_aligned {
                // Raised below; remembered by recomputing there.
            }
        }
        // All the checks have passed, so do the transformation.
        let mut changed_argument = false;
        for index in 0..call_args {
            let argument = self.function.instruction(c).operands[index];
            if self.strip_pointer_casts(argument) == cpy_src {
                changed_argument = true;
                self.function.set_operand(c, index, cpy_dest);
            }
        }
        if !changed_argument {
            return false;
        }
        // The destination, if it was not aligned enough, gets the alignment.
        if src_align > cpy_dest_align
            && let Operand::Value(value) = cpy_dest
            && let llrm_mir::module::ValueDef::Instruction(dest_alloca) = self.function.value(value).def
            && let Opcode::Alloca { allocated: dest_ty, align: dest_align, address_space } =
                self.function.instruction(dest_alloca).opcode
        {
            let current = dest_align.unwrap_or_else(|| self.layout.align(&self.context.types, dest_ty));
            self.function.set_opcode(
                dest_alloca,
                Opcode::Alloca { allocated: dest_ty, align: Some(current.max(src_align)), address_space },
            );
        }
        if move_gep
            && let Operand::Value(value) = cpy_dest
            && let llrm_mir::module::ValueDef::Instruction(gep) = self.function.value(value).def
        {
            self.function
                .move_to(gep, llrm_mir::edit::Position::Before(c))
                .unwrap_or_else(|error| panic!("memcpyopt: {error}"));
        }
        if let Some(start) = skipped_lifetime_start {
            self.function
                .move_to(start, llrm_mir::edit::Position::Before(c))
                .unwrap_or_else(|error| panic!("memcpyopt: {error}"));
            if let (Some(what), Some(before)) = (self.mssa.get_memory_access(start), self.mssa.get_memory_access(c)) {
                self.updater.move_before(&mut self.mssa, &*self.function, what, before);
            }
        }
        true
    }

    /// `PointerMayBeCaptured(object, ReturnCaptures)` for a local object.
    fn may_be_captured(
        &self,
        object: Operand,
    ) -> bool {
        let Operand::Value(value) = object else { return true };
        let result = llrm_analysis::capturetracking::pointer_may_be_captured(
            self.function,
            &self.outer.globals,
            self.context,
            value,
            llrm_analysis::capturetracking::CaptureComponents::ALL,
            |captured| captured != llrm_analysis::capturetracking::CaptureComponents::NONE,
            0,
        );
        result.with_ret != llrm_analysis::capturetracking::CaptureComponents::NONE
    }

    /// `performStackMoveOptzn`: a copy of a whole alloca to another, where the
    /// two never conflict, is the one alloca: the destination's uses move to
    /// the source and the copy goes (the caller erases it). `load` and `store`
    /// are the copy: one memcpy, or a load and its store.
    fn perform_stack_move_optzn(
        &mut self,
        load: InstId,
        store: InstId,
        dest_ptr: Operand,
        src_ptr: Operand,
        size: u64,
    ) -> bool {
        let alloca_of = |this: &Self, ptr: Operand| -> Option<InstId> {
            let Operand::Value(value) = get_underlying_object(&this.query(), ptr, 0) else { return None };
            match this.function.value(value).def {
                llrm_mir::module::ValueDef::Instruction(inst)
                    if matches!(this.function.instruction(inst).opcode, Opcode::Alloca { .. }) =>
                {
                    Some(inst)
                }
                _ => None,
            }
        };
        let (Some(dest_alloca), Some(src_alloca)) = (alloca_of(self, dest_ptr), alloca_of(self, src_ptr)) else {
            return false;
        };
        // A partial copy within one alloca would fail the dominance check.
        if src_alloca == dest_alloca {
            return false;
        }
        let (
            Opcode::Alloca { allocated: src_ty, align: src_align, address_space: src_space },
            Opcode::Alloca { allocated: dest_ty, align: dest_align, address_space: dest_space },
        ) = (
            self.function.instruction(src_alloca).opcode.clone(),
            self.function.instruction(dest_alloca).opcode.clone(),
        )
        else {
            return false;
        };
        if src_space != dest_space {
            return false;
        }
        // Static allocas: a constant count, in the entry block.
        let entry = self.function.layout().first().copied();
        let is_static = |this: &Self, alloca: InstId| {
            this.function.instruction(alloca).operands.first().is_none_or(|count| matches!(count, Operand::Constant(_)))
                && this.function.parent(alloca) == entry
        };
        if !is_static(self, src_alloca) || !is_static(self, dest_alloca) {
            return false;
        }
        let (src_value, dest_value) = (
            Operand::Value(self.function.instruction(src_alloca).result.expect("an alloca has a value")),
            Operand::Value(self.function.instruction(dest_alloca).result.expect("an alloca has a value")),
        );
        let (dest_offset, src_offset) = {
            let q = self.query();
            (get_pointer_offset_from(&q, dest_ptr, dest_value), get_pointer_offset_from(&q, src_ptr, src_value))
        };
        let (Some(dest_offset), Some(src_offset)) = (dest_offset, src_offset) else { return false };
        if src_offset < dest_offset || src_offset < 0 {
            return false;
        }
        let dest_align_bytes = dest_align.unwrap_or_else(|| self.layout.align(&self.context.types, dest_ty));
        let src_align_bytes = src_align.unwrap_or_else(|| self.layout.align(&self.context.types, src_ty));
        // The offset difference must preserve the destination's alignment.
        if (src_offset - dest_offset) as u64 % dest_align_bytes != 0 {
            return false;
        }
        let count = |this: &Self, alloca: InstId| -> Option<u64> {
            match this.function.instruction(alloca).operands.first() {
                None => Some(1),
                Some(&count) => this.query().int_constant(count).map(|(bits, _)| bits as u64),
            }
        };
        let (Some(src_count), Some(dest_count)) = (count(self, src_alloca), count(self, dest_alloca)) else {
            return false;
        };
        let src_size = self.layout.alloc_size(&self.context.types, src_ty) * src_count;
        let dest_size = self.layout.alloc_size(&self.context.types, dest_ty) * dest_count;
        // The copy covers the whole destination, inside the source.
        if size != dest_size || dest_offset != 0 || src_offset as u64 + size > src_size {
            return false;
        }
        // Resizing the source needs the destination's count operand shaped like
        // it.
        if src_size != dest_size
            && self.function.instruction(src_alloca).operands.len()
                != self.function.instruction(dest_alloca).operands.len()
        {
            return false;
        }
        // Combining must not break dominance.
        let move_src = !self.dt.instruction_dominates(self.function, src_alloca, dest_alloca);
        if move_src && !self.dt.instruction_dominates(self.function, dest_alloca, src_alloca) {
            return false;
        }

        // Neither alloca may be captured or escape; the lifetime markers go,
        // and instructions with alias metadata lose it.
        let mut lifetime_markers = Vec::new();
        let mut alias_metadata: Vec<InstId> = Vec::new();
        let (mut dest_mod, mut dest_ref) = (false, false);
        let mut reachability: Vec<llrm_mir::module::BlockId> = Vec::new();
        let store_block = self.function.parent(store).expect("a placed store");
        let view = self.view();
        let aa = AliasQuery::new(&view, &self.accesses);
        let dest_loc = aa.location_for(&self.reference_at(aa.unit(), dest_value, dest_size as u32), false);
        let src_loc = aa.location_for(&self.reference_at(aa.unit(), src_ptr, size as u32), false);

        let mut dest_address_captured = false;
        if !self.track_captures(
            dest_alloca,
            &mut lifetime_markers,
            &mut alias_metadata,
            &mut dest_address_captured,
            &mut |this, user| {
                // The copy itself does not count.
                if user == store {
                    return true;
                }
                let result = aa.get_mod_ref_info(user, &dest_loc);
                dest_mod |= result.is_mod();
                dest_ref |= result.is_ref();
                if result.is_mod_or_ref() {
                    let block = this.function.parent(user).expect("a placed user");
                    if block == store_block {
                        // Before the copy in its block: reachable.
                        if this.function.comes_before(user, store) {
                            return false;
                        }
                        if this.function.entry() == Some(block) {
                            return true;
                        }
                        reachability.extend(this.function.successors(block).iter().copied());
                    } else {
                        reachability.push(block);
                    }
                }
                true
            },
        ) {
            return false;
        }
        // Any mod/ref of the destination before the copy ends it.
        if !reachability.is_empty()
            && llrm_analysis::reachability::is_potentially_reachable_from_many(
                self.function,
                std::mem::take(&mut reachability),
                store_block,
                Some(&self.dt),
            )
        {
            return false;
        }

        // From after the load to the end of the block: a destination written
        // leaves the source unread, a destination read leaves it unwritten.
        let mut post_dominators = llrm_mir::domtree::PostDomTree::new();
        post_dominators.recalculate(&*self.function);
        let post_dominates = |this: &Self, a: InstId, b: InstId| -> bool {
            let (block_a, block_b) =
                (this.function.parent(a).expect("placed"), this.function.parent(b).expect("placed"));
            if block_a != block_b {
                return post_dominators.dominates(block_a, block_b);
            }
            this.function.comes_before(b, a)
        };
        let mut src_address_captured = false;
        let mut ignored_markers = Vec::new();
        let mut ignored_metadata = Vec::new();
        if !self.track_captures(
            src_alloca,
            &mut ignored_markers,
            &mut ignored_metadata,
            &mut src_address_captured,
            &mut |this, user| {
                // Anything the load post-dominates, and the copy, do not
                // matter.
                if user == load || user == store || post_dominates(this, load, user) {
                    return true;
                }
                let result = aa.get_mod_ref_info(user, &src_loc);
                !((dest_mod && result.is_ref()) || (dest_ref && result.is_mod()))
            },
        ) {
            return false;
        }
        lifetime_markers.extend(ignored_markers);
        alias_metadata.extend(ignored_metadata);
        // With both addresses captured, being one object could be seen.
        if dest_address_captured && src_address_captured {
            return false;
        }

        // The transformation. First the source moves up if it was after the
        // destination.
        if move_src {
            self.function
                .move_to(src_alloca, llrm_mir::edit::Position::Before(dest_alloca))
                .unwrap_or_else(|error| panic!("memcpyopt: {error}"));
        }
        // Align and size the merged alloca.
        let merged_align = src_align_bytes.max(dest_align_bytes);
        let (merged_ty, merged_count) = if dest_size > src_size {
            (dest_ty, self.function.instruction(dest_alloca).operands.first().copied())
        } else {
            (src_ty, None)
        };
        self.function.set_opcode(
            src_alloca,
            Opcode::Alloca { allocated: merged_ty, align: Some(merged_align), address_space: src_space },
        );
        if let Some(count) = merged_count
            && !self.function.instruction(src_alloca).operands.is_empty()
        {
            self.function.set_operand(src_alloca, 0, count);
        }
        // Merge: the destination's uses go to the source, offset if need be.
        let new_dest = if src_offset != dest_offset {
            let bits = self.layout.pointer(src_space).bits;
            let index =
                Operand::Constant(self.context.int(self.context.types.int(bits), i128::from(src_offset - dest_offset)));
            let i8 = self.context.types.int(8);
            self.insert_before(
                dest_alloca,
                Opcode::GetElementPtr { source: i8 },
                self.function.value(self.operand_value(src_value)).ty,
                vec![src_value, index],
                Flags::INBOUNDS,
            )
        } else {
            src_value
        };
        self.function.replace_all_uses_with(self.operand_value(dest_value), new_dest);
        self.erase_instruction(dest_alloca);
        // The merged alloca keeps only its debug metadata.
        let kept: Vec<_> = self
            .function
            .instruction(src_alloca)
            .metadata
            .iter()
            .filter(|(kind, _)| kind.starts_with("dbg"))
            .cloned()
            .collect();
        for kind in
            self.function.instruction(src_alloca).metadata.iter().map(|(kind, _)| kind.clone()).collect::<Vec<_>>()
        {
            if !kept.iter().any(|(one, _)| *one == kind) {
                self.function.set_metadata(src_alloca, &kind, None);
            }
        }
        for marker in lifetime_markers {
            if !self.function.is_erased(marker) {
                self.erase_instruction(marker);
            }
        }
        // Accesses that did not alias now may: their alias metadata goes.
        for inst in alias_metadata {
            if self.function.is_erased(inst) {
                continue;
            }
            for kind in ["alias.scope", "noalias", "tbaa", "tbaa_struct"] {
                self.function.set_metadata(inst, kind, None);
            }
        }
        true
    }

    /// `CaptureTrackingWithModRef`: follows the uses of `alloca` through what
    /// keeps its result; false where one captures its provenance, where
    /// `mod_ref` says false of an instruction that touches memory, or where
    /// there are too many uses to see. The lifetime markers and the other
    /// memory-touching users are collected.
    fn track_captures(
        &self,
        alloca: InstId,
        lifetime_markers: &mut Vec<InstId>,
        alias_metadata: &mut Vec<InstId>,
        address_captured: &mut bool,
        mod_ref: &mut dyn FnMut(&Self, InstId) -> bool,
    ) -> bool {
        use llrm_analysis::capturetracking::{CaptureComponents, DEFAULT_MAX_USES_TO_EXPLORE};
        let base = self.function.instruction(alloca).result.expect("an alloca has a value");
        let mut worklist = vec![base];
        let mut visited: llrm_support::hash::HashSet<(InstId, u32)> = Default::default();
        while let Some(value) = worklist.pop() {
            for &used in self.function.users(value) {
                if visited.len() >= DEFAULT_MAX_USES_TO_EXPLORE {
                    return false;
                }
                if !visited.insert((used.user, used.index)) {
                    continue;
                }
                let info = llrm_analysis::capturetracking::determine_use_capture_kind(
                    self.function,
                    &self.outer.globals,
                    self.context,
                    used,
                    base,
                );
                if info.use_cc.intersect(CaptureComponents::PROVENANCE).captures_anything() {
                    return false;
                }
                *address_captured |= info.use_cc.intersect(CaptureComponents::ADDRESS).captures_anything();
                if self.mssa.get_memory_access(used.user).is_some() {
                    if matches!(
                        self.intrinsic_of(used.user),
                        Some(Intrinsic::LifetimeStart | Intrinsic::LifetimeEnd)
                    ) {
                        lifetime_markers.push(used.user);
                        continue;
                    }
                    alias_metadata.push(used.user);
                    if !mod_ref(self, used.user) {
                        return false;
                    }
                }
                if info.result_cc.captures_anything()
                    && let Some(result) = self.function.instruction(used.user).result
                {
                    worklist.push(result);
                }
            }
        }
        true
    }

    /// The value an operand names.
    fn operand_value(
        &self,
        operand: Operand,
    ) -> llrm_mir::module::ValueId {
        let Operand::Value(value) = operand else { panic!("memcpyopt: a value") };
        value
    }

    /// `processStore`, its `processStoreOfLoad` arm: the store of a loaded
    /// value. The memset arms are stage 1's (`crate::memcpyoptimizer`).
    fn process_store(
        &mut self,
        si: InstId,
        bbi: &mut Option<InstId>,
    ) -> bool {
        let Opcode::Store { volatile: false, .. } = self.function.instruction(si).opcode else { return false };
        // A copy would lose the non-temporal hint.
        if self.function.instruction(si).metadata.iter().any(|(kind, _)| kind == "nontemporal") {
            return false;
        }
        let Operand::Value(stored) = self.function.instruction(si).operands[0] else { return false };
        let llrm_mir::module::ValueDef::Instruction(li) = self.function.value(stored).def else { return false };
        if !matches!(self.function.instruction(li).opcode, Opcode::Load { .. }) {
            return false;
        }
        self.process_store_of_load(si, li, bbi)
    }

    /// `MemoryLocation::get(I)` of a load or a store.
    fn access_location(
        &self,
        aa: &AliasQuery,
        inst: InstId,
    ) -> Option<Rc<Location>> {
        let instruction = self.function.instruction(inst);
        let (pointer, value) = match instruction.opcode {
            Opcode::Load { .. } => (instruction.operands[0], instruction.result.map(Operand::Value)?),
            Opcode::Store { .. } => (instruction.operands[1], instruction.operands[0]),
            _ => return None,
        };
        let ty = self.function.operand_type(self.context, value)?;
        let width = u32::try_from(self.layout.store_size(&self.context.types, ty)).ok()?;
        Some(aa.location_for(&self.reference_at(aa.unit(), pointer, width), false))
    }

    /// `processStoreOfLoad`: `li` loaded what `si` stores. An aggregate is
    /// copied in one go; otherwise the pair may be a call's result being moved
    /// out of its slot. The stack move is a later stage.
    fn process_store_of_load(
        &mut self,
        si: InstId,
        li: InstId,
        bbi: &mut Option<InstId>,
    ) -> bool {
        let Opcode::Load { volatile: false, align: load_align } = self.function.instruction(li).opcode else {
            return false;
        };
        let Opcode::Store { align: store_align, .. } = self.function.instruction(si).opcode else { return false };
        let result = self.function.instruction(li).result.expect("a load has a value");
        if !self.function.users(result).has_one_use() || self.function.parent(li) != self.function.parent(si) {
            return false;
        }
        let ty = self.function.value(result).ty;
        let size = self.layout.store_size(&self.context.types, ty);
        // `getAlign()`: an access with none is aligned as its type is.
        let abi_align = self.layout.align(&self.context.types, ty);
        let (load_align, store_align) = (load_align.unwrap_or(abi_align), store_align.unwrap_or(abi_align));
        if matches!(
            self.context.types.get(ty),
            llrm_mir::types::Type::Struct { .. }
                | llrm_mir::types::Type::Named(_)
                | llrm_mir::types::Type::Array { .. }
        ) && self.copy_aggregate(si, li, size, load_align, store_align, bbi)
        {
            return true;
        }
        // Call slot forwarding through a load-store pair.
        let (dest, source) = (self.function.instruction(si).operands[1], self.function.instruction(li).operands[0]);
        let (dest, source) = (self.strip_pointer_casts(dest), self.strip_pointer_casts(source));
        let get_c = |this: &Self| {
            let view = this.view();
            let aa = AliasQuery::new(&view, &this.accesses);
            let access = this.mssa.get_memory_access(li)?;
            let defining = this.mssa.access(access).defining?;
            let loc = this.access_location(&aa, li)?;
            let mut limit = llrm_analysis::mssa::walker::MAX_CHECK_LIMIT;
            let clobber = this.mssa.get_clobbering_memory_access_for_location(&aa, defining, &loc, &mut limit);
            let inst = this.mssa.access(clobber).inst?;
            matches!(this.function.instruction(inst).opcode, Opcode::Call(_)).then_some(inst)
        };
        if self.perform_call_slot_optzn(li, si, dest, source, size, store_align.min(load_align), get_c) {
            self.erase_instruction(si);
            self.erase_instruction(li);
            return true;
        }
        // A load-store pair from one stack slot to another.
        let (store_ptr, load_ptr) =
            (self.function.instruction(si).operands[1], self.function.instruction(li).operands[0]);
        if self.perform_stack_move_optzn(li, si, store_ptr, load_ptr, size) {
            *bbi = self.function.next_in_block(si);
            self.erase_instruction(si);
            self.erase_instruction(li);
            return true;
        }
        false
    }

    /// The aggregate arm of `processStoreOfLoad`: the load and the store are
    /// one memcpy, or memmove where the store may write what the load reads,
    /// put where the loaded bytes were last written.
    fn copy_aggregate(
        &mut self,
        si: InstId,
        li: InstId,
        size: u64,
        load_align: u64,
        store_align: u64,
        bbi: &mut Option<InstId>,
    ) -> bool {
        // The first write to the loaded bytes between the load and the store:
        // the copy goes there instead of at the store.
        let (p, store_writes_load) = {
            let view = self.view();
            let aa = AliasQuery::new(&view, &self.accesses);
            let Some(load_loc) = self.access_location(&aa, li) else { return false };
            let mut p = si;
            let mut at = self.function.next_in_block(li);
            while let Some(inst) = at.filter(|&one| one != si) {
                if aa.get_mod_ref_info(inst, &load_loc).is_mod() {
                    p = inst;
                    break;
                }
                at = self.function.next_in_block(inst);
            }
            (p, aa.get_mod_ref_info(si, &load_loc).is_mod())
        };
        if p != si && !self.move_up(si, p, li) {
            return false;
        }
        let length = {
            let i64 = self.context.types.int(64);
            Operand::Constant(self.context.int(i64, i128::from(size)))
        };
        let (dest, source) = (self.function.instruction(si).operands[1], self.function.instruction(li).operands[0]);
        // A store that may write what the load reads needs the move.
        let how = if store_writes_load { Copy::Memmove } else { Copy::Memcpy };
        let new_m = create_mem_copy(
            &mut *self.context,
            &mut *self.declared,
            &mut *self.function,
            p,
            how,
            dest,
            Some(store_align),
            source,
            Some(load_align),
            length,
        );
        let last_def = self.mssa.get_memory_access(si).expect("a store has a def");
        self.insert_def(new_m, Created::After(last_def), true);
        self.erase_instruction(si);
        self.erase_instruction(li);
        // Do not invalidate the iterator.
        *bbi = Some(new_m);
        true
    }

    /// `moveUp`: lifts `si`, and what it needs, before `p`, if nothing between
    /// them stops it. The load `li` is implicitly moved down past what is
    /// lifted, so none of it may write what `li` reads.
    fn move_up(
        &mut self,
        si: InstId,
        p: InstId,
        li: InstId,
    ) -> bool {
        let block = self.function.parent(si).expect("a placed store");
        let mut to_lift = vec![si];
        let mut args: llrm_mir::dense::IdSet<InstId> = Default::default();
        // The in-block definition of `operand`; `None` for another block's, or
        // none.
        let in_block_def = |this: &Self, operand: Operand| -> Option<InstId> {
            let Operand::Value(value) = operand else { return None };
            match this.function.value(value).def {
                llrm_mir::module::ValueDef::Instruction(def) if this.function.parent(def) == Some(block) => Some(def),
                _ => None,
            }
        };
        // `AddArg`: false when a lifted instruction uses `p` itself.
        let add_arg = |this: &Self, operand: Operand, args: &mut llrm_mir::dense::IdSet<InstId>| -> bool {
            if let Some(def) = in_block_def(this, operand) {
                if def == p {
                    return false;
                }
                args.insert(def);
            }
            true
        };
        {
            let view = self.view();
            let aa = AliasQuery::new(&view, &self.accesses);
            let Some(store_loc) = self.access_location(&aa, si) else { return false };
            // If the store aliases this position, bail out.
            if aa.get_mod_ref_info(p, &store_loc).is_mod_or_ref() {
                return false;
            }
            let Some(load_loc) = self.access_location(&aa, li) else { return false };
            if !add_arg(self, self.function.instruction(si).operands[1], &mut args) {
                return false;
            }
            let mut locations = vec![store_loc];
            let mut at = self.function.prev_in_block(si);
            while let Some(c) = at.filter(|&one| one != p) {
                at = self.function.prev_in_block(c);
                // Make sure hoisting does not perform a store that was not
                // guaranteed to happen.
                if !llrm_mir::valuetracking::guarantee::is_guaranteed_to_transfer_execution_to_successor(
                    &self.query(),
                    c,
                ) {
                    return false;
                }
                // `getModRefInfo(C, std::nullopt)`: it has a memory access.
                let may_alias = self.mssa.get_memory_access(c).is_some();
                let need_lift = if args.remove(&c) {
                    true
                } else {
                    may_alias && locations.iter().any(|loc| aa.get_mod_ref_info(c, loc).is_mod_or_ref())
                };
                if !need_lift {
                    continue;
                }
                if may_alias {
                    // None of the lifted may modify the load's source.
                    if aa.get_mod_ref_info(c, &load_loc).is_mod() {
                        return false;
                    }
                    match self.function.instruction(c).opcode {
                        Opcode::Load { .. } | Opcode::Store { .. } => {
                            let Some(loc) = self.access_location(&aa, c) else { return false };
                            // If we can't lift this before `p`, it's game over.
                            if aa.get_mod_ref_info(p, &loc).is_mod_or_ref() {
                                return false;
                            }
                            locations.push(loc);
                        }
                        // We don't know how to lift this instruction (a call:
                        // DEVIATE 6).
                        _ => return false,
                    }
                }
                to_lift.push(c);
                for operand in self.function.instruction(c).operands.clone() {
                    if !add_arg(self, operand, &mut args) {
                        return false;
                    }
                }
            }
        }
        // The point in the memory graph the lifted accesses follow: the access
        // before `p`'s, or the nearest one above `p` back to the load.
        let mut insert_point = if let Some(access) = self.mssa.get_memory_access(p) {
            let list = self.mssa.get_block_accesses(block).expect("a placed access");
            let at = list.iter().position(|&one| one == access).expect("a placed access");
            Some(list[at - 1])
        } else {
            let mut found = None;
            let mut at = self.function.prev_in_block(p);
            while let Some(inst) = at {
                if let Some(access) = self.mssa.get_memory_access(inst) {
                    found = Some(access);
                    break;
                }
                if inst == li {
                    break;
                }
                at = self.function.prev_in_block(inst);
            }
            found
        };
        // We made it, we need to lift.
        for inst in to_lift.into_iter().rev() {
            self.function
                .move_to(inst, llrm_mir::edit::Position::Before(p))
                .unwrap_or_else(|error| panic!("memcpyopt: {error}"));
            if let Some(access) = self.mssa.get_memory_access(inst) {
                let point = insert_point.expect("a memory access to insert after");
                self.updater.move_after(&mut self.mssa, &*self.function, access, point);
                insert_point = Some(access);
            }
        }
        true
    }

    /// `isMemMoveMemSetDependency`: the memmove moves a memset's bytes onto
    /// themselves.
    fn is_memmove_memset_dependency(
        &self,
        m: &Mem,
    ) -> bool {
        let Some(access) = self.mssa.get_memory_access(m.inst) else { return false };
        let Some(move_size) = self.constant_length(m.length) else { return false };
        // The source is a constant offset of the destination.
        let q = self.query();
        let Some((_, source)) = q.instruction_of(m.other) else { return false };
        if !matches!(source.opcode, Opcode::GetElementPtr { .. }) || source.operands[0] != m.dest {
            return false;
        }
        let Some(offset) = get_pointer_offset_from(&q, m.other, m.dest).filter(|&offset| offset >= 0) else {
            return false;
        };
        let view = self.view();
        let aa = AliasQuery::new(&view, &self.accesses);
        let total = u32::try_from(offset as u64 + move_size).ok();
        let reference = self.reference_at(aa.unit(), m.dest, total.unwrap_or(1));
        let combined = aa.location_for(&reference, total.is_none());
        let first_def = self.mssa.access(access).defining.expect("a defining access");
        let mut limit = llrm_analysis::mssa::walker::MAX_CHECK_LIMIT;
        let clobber = self.mssa.get_clobbering_memory_access_for_location(&aa, first_def, &combined, &mut limit);
        if self.mssa.access(clobber).kind != Kind::Def {
            return false;
        }
        let Some(set) =
            self.mssa.access(clobber).inst.and_then(|inst| self.mem(inst)).filter(|one| one.kind == Kind3::Set)
        else {
            return false;
        };
        let Some(set_length) = self.constant_length(set.length) else { return false };
        if set_length < offset as u64 + move_size {
            return false;
        }
        self.must_alias(&aa, set.dest, m.dest)
    }

    /// The volatile flag of a new memory intrinsic call.
    fn set_volatile(
        &mut self,
        call: InstId,
        volatile: bool,
    ) {
        if !volatile {
            return;
        }
        let i1 = self.context.types.int(1);
        let one = Operand::Constant(self.context.int(i1, 1));
        self.function.set_operand(call, 3, one);
    }

    /// `writtenBetween`, asked of the shared graph (the walker's by-location
    /// walk reads it only).
    fn written_between_shared(
        &self,
        aa: &AliasQuery,
        loc: &Rc<Location>,
        start: AccessId,
        end: AccessId,
    ) -> bool {
        if self.mssa.access(end).kind == Kind::Use {
            let block = self.mssa.access(start).block;
            if block != self.mssa.access(end).block {
                return true;
            }
            let list = self.mssa.get_block_accesses(block).expect("a placed access");
            let from = list.iter().position(|&one| one == start).expect("a placed access") + 1;
            let to = list.iter().position(|&one| one == end).expect("a placed access");
            return list[from..to]
                .iter()
                .any(
                    |&access| {
                        if self.mssa.access(access).kind == Kind::Use {
                            return false;
                        }
                        let inst = self.mssa.access(access).inst.expect("a def has an instruction");
                        aa.get_mod_ref_info(inst, loc).is_mod()
                    },
                );
        }
        let defining = self.mssa.access(end).defining.expect("a defining access");
        let mut limit = llrm_analysis::mssa::walker::MAX_CHECK_LIMIT;
        let clobber = self.mssa.get_clobbering_memory_access_for_location(aa, defining, loc, &mut limit);
        !self.mssa.dominates(clobber, start)
    }

    /// `isBytewiseValue`'s byte as the operand a memset takes.
    fn byte_operand(
        &self,
        byte: Byte,
    ) -> Operand {
        let i8 = self.context.types.int(8);
        match byte {
            Byte::Constant(bits) => Operand::Constant(self.context.int(i8, i128::from(bits))),
            Byte::Value(value) => value,
            Byte::Undef => Operand::Constant(self.context.constant(Constant { ty: i8, kind: ConstantKind::Undef })),
            Byte::Poison => Operand::Constant(self.context.constant(Constant { ty: i8, kind: ConstantKind::Poison })),
        }
    }

    /// The attributes of argument `index` of `call`: the call site's and its
    /// callee's declaration's (`CallBase::paramHasAttr`).
    fn argument_attrs(
        &self,
        call: InstId,
        index: usize,
    ) -> Vec<llrm_mir::opcode::Attribute> {
        let Opcode::Call(info) = &self.function.instruction(call).opcode else { return Vec::new() };
        let mut attrs = info.argument_attrs.get(index).cloned().unwrap_or_default();
        if let Some(callee) = llrm_mir::memory::callee(self.context, self.function, call)
            && let Some(function) = self.outer.globals.get(callee.0 as usize).and_then(|global| global.function())
        {
            attrs.extend(function.parameter_attrs.get(index).cloned().unwrap_or_default());
        }
        attrs
    }

    /// The alignment `pointer` is known to have: an alloca's. DEVIATE 9: the
    /// pin's `getOrEnforceKnownAlignment` also raises the alignment of an
    /// alloca or global it may; here a smaller known alignment ends the
    /// transform.
    fn known_alignment(
        &self,
        pointer: Operand,
    ) -> u64 {
        let Operand::Value(value) = get_underlying_object(&self.query(), pointer, 0) else { return 1 };
        let llrm_mir::module::ValueDef::Instruction(inst) = self.function.value(value).def else { return 1 };
        let Opcode::Alloca { allocated, align, .. } = self.function.instruction(inst).opcode else { return 1 };
        align.unwrap_or_else(|| self.layout.align(&self.context.types, allocated))
    }

    /// The memcpy that fed `pointer` (`size` bytes of it) to `call`, if the
    /// clobber of that location right before the call is one.
    fn feeding_memcpy(
        &self,
        aa: &AliasQuery,
        call: InstId,
        pointer: Operand,
        size: u64,
    ) -> Option<Mem> {
        let access = self.mssa.get_memory_access(call)?;
        let defining = self.mssa.access(access).defining?;
        let location = aa.location_for(&self.reference_at(aa.unit(), pointer, size as u32), false);
        let mut limit = llrm_analysis::mssa::walker::MAX_CHECK_LIMIT;
        let clobber = self.mssa.get_clobbering_memory_access_for_location(aa, defining, &location, &mut limit);
        if self.mssa.access(clobber).kind != Kind::Def {
            return None;
        }
        self.mem(self.mssa.access(clobber).inst?).filter(|one| one.kind == Kind3::Copy && !one.volatile)
    }

    /// Whether the bytes `dep` copied from are written between the copy and
    /// `call`.
    fn source_written_between(
        &self,
        aa: &AliasQuery,
        dep: &Mem,
        call: InstId,
    ) -> bool {
        let (Some(start), Some(end)) = (self.mssa.get_memory_access(dep.inst), self.mssa.get_memory_access(call))
        else {
            return true;
        };
        let Some(source) = self.source_location(aa, dep) else { return true };
        self.written_between_shared(aa, &source, start, end)
    }

    /// `processByValArgument`: a `byval` argument fed by a memcpy takes the
    /// memcpy's source instead.
    fn process_byval_argument(
        &mut self,
        call: InstId,
        index: usize,
    ) -> bool {
        let attrs = self.argument_attrs(call, index);
        let Some(by_val_ty) = attrs.iter().find_map(|attr| match attr {
            llrm_mir::opcode::Attribute::Type(name, ty) if name == "byval" => Some(*ty),
            _ => None,
        }) else {
            return false;
        };
        let size = self.layout.alloc_size(&self.context.types, by_val_ty);
        let argument = self.function.instruction(call).operands[index];
        // The call doesn't say its alignment: a target-specific value.
        let Some(by_val_align) = llrm_mir::facts::Facts::of(&attrs).align() else { return false };
        let dep = {
            let view = self.view();
            let aa = AliasQuery::new(&view, &self.accesses);
            let Some(dep) = self.feeding_memcpy(&aa, call, argument, size) else { return false };
            // The argument is the memcpy's destination, which covers it.
            if self.strip_pointer_casts(argument) != dep.dest {
                return false;
            }
            if !self.constant_length(dep.length).is_some_and(|length| length >= size) {
                return false;
            }
            let dep_align = self.align_of(dep.inst, 1).unwrap_or(1).max(self.known_alignment(dep.other));
            if dep_align < by_val_align {
                return false;
            }
            if self.function.operand_type(self.context, dep.other) != self.function.operand_type(self.context, argument)
            {
                return false;
            }
            if self.source_written_between(&aa, &dep, call) {
                return false;
            }
            dep
        };
        self.function.set_operand(call, index, dep.other);
        true
    }

    /// `processImmutArgument`: a pointer argument the callee only reads, to a
    /// whole alloca a memcpy filled, is the memcpy's source instead.
    fn process_immut_argument(
        &mut self,
        call: InstId,
        index: usize,
    ) -> bool {
        use llrm_mir::facts::Facts;
        let attrs = self.argument_attrs(call, index);
        let facts = Facts::of(&attrs);
        // 1. Immutable during the call, and not captured by it.
        if !facts.no_capture() {
            return false;
        }
        let argument = self.function.instruction(call).operands[index];
        let dep = {
            let view = self.view();
            let aa = AliasQuery::new(&view, &self.accesses);
            // The callee may still write it through another pointer: noalias,
            // or the alias model says not.
            if !facts.no_alias() {
                let any = aa.location_for(&self.reference_at(aa.unit(), argument, 1), true);
                if aa.get_mod_ref_info(call, &any).is_mod() {
                    return false;
                }
            }
            // 2. An alloca of a size known.
            let Operand::Value(alloca_value) = self.strip_pointer_casts(argument) else { return false };
            let llrm_mir::module::ValueDef::Instruction(alloca) = self.function.value(alloca_value).def else {
                return false;
            };
            let Opcode::Alloca { allocated, align, .. } = self.function.instruction(alloca).opcode else {
                return false;
            };
            let count = match self.function.instruction(alloca).operands.first() {
                None => 1,
                Some(&count) => match self.query().int_constant(count) {
                    Some((bits, _)) => bits as u64,
                    None => return false,
                },
            };
            let alloca_size = self.layout.alloc_size(&self.context.types, allocated) * count;
            let Some(dep) = self.feeding_memcpy(&aa, call, argument, alloca_size) else { return false };
            if dep.dest != Operand::Value(alloca_value) {
                return false;
            }
            if self.function.operand_type(self.context, dep.other) != self.function.operand_type(self.context, argument)
            {
                return false;
            }
            // 2-1. The copy is the whole alloca.
            if self.constant_length(dep.length) != Some(alloca_size) {
                return false;
            }
            // 2-2. The source is aligned as the alloca is.
            let alloca_align = align.unwrap_or_else(|| self.layout.align(&self.context.types, allocated));
            let dep_align = self.align_of(dep.inst, 1).unwrap_or(1).max(self.known_alignment(dep.other));
            if dep_align < alloca_align {
                return false;
            }
            // 3. The source is not written between the copy and the call.
            if self.source_written_between(&aa, &dep, call) {
                return false;
            }
            // 4. Nor during the call.
            let Some(source) = self.source_location(&aa, &dep) else { return false };
            if aa.get_mod_ref_info(call, &source).is_mod() {
                return false;
            }
            dep
        };
        self.function.set_operand(call, index, dep.other);
        true
    }

    /// The alignment an argument of a call carries (`align` on parameter
    /// `index`).
    fn align_of(
        &self,
        call: InstId,
        index: usize,
    ) -> Option<u64> {
        let Opcode::Call(info) = &self.function.instruction(call).opcode else { return None };
        llrm_mir::facts::Facts::of(info.argument_attrs.get(index)?).align()
    }
}

/// `commonAlignment(A, Offset)`: the alignment of `offset` bytes past an
/// aligned address.
fn common_alignment(
    align: u64,
    offset: u64,
) -> u64 {
    if offset == 0 { align } else { align.min(1 << offset.trailing_zeros()) }
}

#[cfg(test)]
#[path = "memcpyopt_tests.rs"]
mod tests;
