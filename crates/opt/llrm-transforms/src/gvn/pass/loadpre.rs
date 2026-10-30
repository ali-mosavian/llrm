//! Load PRE: a load available on some of the paths into its block becomes
//! available on all of them, by a load inserted in the predecessors that lack
//! it (`performLoadPRE`), or, for a loop header's load whose address is
//! invariant, by one in the preheader (`performLoopLoadPRE`).
//!
//! A translation of
//! [`GVN.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp)
//! at d1106deb71cc, with `enable-load-pre`, `enable-load-in-loop-pre` on and
//! `enable-load-pre-split-backedge` off (the defaults).
//!
//! DEVIATE 20, `isSafeToSpeculativelyExecute(Load, CtxI, AC, DT)` is asked of
//! the load where it stands
//! (`licm::hoist::is_safe_to_speculatively_execute_in`): the context-sensitive
//! dereferenceability facts at `CtxI` are not used. DEVIATE 21,
//! `Value::canBeFreed` is true for every pointer but an `alloca` or a global
//! variable (and what is a GEP, cast or phi of them): MIR has no `nofree`
//! dereferenceability to read, so a loop load of an argument or a loaded
//! pointer is not moved to the preheader.
//! DEVIATE 22, `ImplicitControlFlowTracking` is asked by scanning the block (a
//! block is scanned for each PRE candidate, not each instruction).

use llrm_mir::dense::IdMap;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{Flags, Opcode};

use super::GvnPass;
use super::loads::{AvailableValue, AvailableValueInBlock, Kind};

/// `gvn-max-block-speculations`.
const MAX_BB_SPECULATIONS: usize = 600;
/// `gvn-max-num-insns`.
const MAX_NUM_INSNS_PER_BLOCK: u32 = 100;

/// `AvailabilityState`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Availability {
    /// We know the block is not fully available. This is a fixpoint.
    Unavailable,
    /// We know the block is fully available. This is a fixpoint.
    Available,
    /// We do not know whether the block is fully available, but we are
    /// speculating that it will be.
    SpeculativelyAvailable,
}

/// The metadata `eliminatePartiallyRedundantLoad` gives the inserted load: the
/// AA tags, `invariant.load`, `invariant.group`, `range` and `nofpclass`.
const COPIED_METADATA: [&str; 9] = [
    "tbaa",
    "tbaa.struct",
    "alias.scope",
    "noalias",
    "invariant.load",
    "invariant.group",
    "range",
    "nofpclass",
    "noalias.addrspace",
];

impl GvnPass<'_> {
    /// `isValueFullyAvailableInBlock`
    /// ([GVN.cpp:970](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L970)):
    /// whether the value is available at the end of `bb` on every path into it,
    /// speculating about the blocks not yet decided.
    fn is_value_fully_available_in_block(
        &self,
        bb: BlockId,
        fully_available_blocks: &mut IdMap<BlockId, Availability>,
    ) -> bool {
        let mut worklist = vec![bb];
        let mut unavailable_bb: Option<BlockId> = None;
        let mut num_new_speculatively_available_bbs = 0usize;
        while let Some(curr) = worklist.pop() {
            // Depth-first.
            let state = fully_available_blocks.get(&curr).copied();
            match state {
                Some(Availability::Unavailable) => {
                    // Backpropagate unavailability info.
                    unavailable_bb = Some(curr);
                    break;
                }
                // Don't recurse further, but continue processing the worklist.
                Some(_) => continue,
                None => {
                    fully_available_blocks.insert(curr, Availability::SpeculativelyAvailable);
                }
            }
            num_new_speculatively_available_bbs += 1;
            let out_of_budget = num_new_speculatively_available_bbs > MAX_BB_SPECULATIONS;
            let preds = self.function.predecessors(curr);
            if out_of_budget || preds.is_empty() {
                fully_available_blocks.insert(curr, Availability::Unavailable);
                unavailable_bb = Some(curr);
                break;
            }
            worklist.extend(preds.iter().copied());
        }
        // Propagate the verdict to the speculation that depended on it.
        if let Some(unavailable) = unavailable_bb {
            worklist.clear();
            worklist.extend(self.function.successors(unavailable).iter().copied());
            while let Some(block) = worklist.pop() {
                // Never queried this block, leave it as it is.
                if fully_available_blocks.get(&block) == Some(&Availability::SpeculativelyAvailable) {
                    fully_available_blocks.insert(block, Availability::Unavailable);
                    worklist.extend(self.function.successors(block).iter().copied());
                }
            }
        }
        unavailable_bb.is_none()
    }

    /// `ImplicitControlFlowTracking::hasICF`, by a scan (DEVIATE 22).
    fn has_icf(
        &self,
        bb: BlockId,
    ) -> bool {
        self.function.iter_block(bb).any(|inst| !self.transfers_execution(inst))
    }

    /// `isDominatedByICFIFromSameBlock`: some instruction before `inst` in its
    /// block need not return to its successor.
    pub(super) fn is_dominated_by_icf_from_same_block(
        &self,
        inst: InstId,
    ) -> bool {
        let Some(block) = self.function.parent(inst) else { return false };
        self.function.iter_block(block).take_while(|&one| one != inst).any(|one| !self.transfers_execution(one))
    }

    fn transfers_execution(
        &self,
        inst: InstId,
    ) -> bool {
        crate::local::is_guaranteed_to_transfer_execution_to_successor(
            self.context,
            &self.outer.globals,
            self.function,
            inst,
        )
    }

    /// `Value::canBeFreed` (DEVIATE 21).
    fn can_be_freed(
        &self,
        pointer: Operand,
    ) -> bool {
        let (base, _) = llrm_mir::valuetracking::underlying(self.context, self.layout, self.function, pointer);
        match base {
            Operand::Constant(constant) => !matches!(
                self.context.get(constant).kind,
                llrm_mir::context::ConstantKind::Global(global) if matches!(self.outer.globals.get(global.0 as usize).map(|one| &one.kind), Some(llrm_mir::module::GlobalKind::Variable(_)))
            ),
            Operand::Value(value) => match self.function.value(value).def {
                llrm_mir::module::ValueDef::Instruction(inst) => {
                    !matches!(self.function.instruction(inst).opcode, Opcode::Alloca { .. })
                }
                llrm_mir::module::ValueDef::Argument(_) => true,
            },
            Operand::Block(_) => true,
        }
    }

    /// `findLoadToHoistIntoPred`
    /// ([GVN.cpp:1559](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1559)):
    /// a load identical to `load` in the other successor of `pred`, which has
    /// no dependence in its block, so it can move into `pred`.
    fn find_load_to_hoist_into_pred(
        &mut self,
        pred: BlockId,
        load_bb: BlockId,
        load: InstId,
    ) -> Option<InstId> {
        let term = self.function.terminator(pred)?;
        // A conditional branch, whose two successors differ from a special one.
        let terminator = self.function.instruction(term);
        if !matches!(terminator.opcode, Opcode::Br) || terminator.operands.len() != 3 {
            return None;
        }
        let [_, Operand::Block(first), Operand::Block(second)] = terminator.operands[..] else { return None };
        let succ_bb = if first == load_bb { second } else { first };
        // The other successor must have a single predecessor.
        crate::edges::single_predecessor(self.function, succ_bb)?;
        let mut num_insts = MAX_NUM_INSNS_PER_BLOCK;
        for inst in self.function.iter_block(succ_bb).collect::<Vec<_>>() {
            num_insts -= 1;
            if num_insts == 0 {
                return None;
            }
            if !self.is_identical_to(inst, load) {
                continue;
            }
            // A load with no local dependence is the one to hoist.
            let has_local_dep = !self.md_get_dependency(inst).is_non_local();
            if !has_local_dep && !self.is_dominated_by_icf_from_same_block(inst) {
                return Some(inst);
            }
            return None;
        }
        None
    }

    /// `Instruction::isIdenticalTo` of two loads.
    fn is_identical_to(
        &self,
        a: InstId,
        b: InstId,
    ) -> bool {
        let (a, b) = (self.function.instruction(a), self.function.instruction(b));
        a.opcode == b.opcode && a.ty == b.ty && a.operands == b.operands && a.flags == b.flags
    }

    /// `replaceValuesPerBlockEntry`
    /// ([GVN.cpp:1086](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1086)):
    /// the available values that were `old` are `new`.
    fn replace_values_per_block_entry(
        &self,
        values_per_block: &mut [AvailableValueInBlock],
        old: InstId,
        new: InstId,
    ) {
        let old_value = Operand::Value(self.function.instruction(old).result.expect("a load"));
        let new_value = Operand::Value(self.function.instruction(new).result.expect("a load"));
        for one in values_per_block {
            match &mut one.av.kind {
                Kind::Simple(value) if *value == old_value => *value = new_value,
                Kind::Load(load) if *load == old => *load = new,
                Kind::Select { cond, v1, v2 } => {
                    for slot in [cond, v1, v2] {
                        if *slot == old_value {
                            *slot = new_value;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// `eliminatePartiallyRedundantLoad`
    /// ([GVN.cpp:1631](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1631)):
    /// a load of the address in each of `available_loads`' blocks, the values
    /// merged, and `load` replaced.
    fn eliminate_partially_redundant_load(
        &mut self,
        load: InstId,
        values_per_block: &mut Vec<AvailableValueInBlock>,
        available_loads: &[(BlockId, Operand)],
        critical_edge_pred_and_load: Option<&[(BlockId, InstId)]>,
    ) {
        let load_instruction = self.function.instruction(load).clone();
        let load_name = load_instruction.result.and_then(|value| self.function.value(value).name.clone());
        let load_block = self.function.parent(load).expect("a placed load");
        for &(unavailable_block, load_ptr) in available_loads {
            let name = load_name.as_ref().map(|name| format!("{name}.pre"));
            let new_load = self
                .function
                .create_instruction(
                    load_instruction.opcode.clone(),
                    load_instruction.ty,
                    vec![load_ptr],
                    Flags::default(),
                    name.as_deref(),
                );
            let terminator = self.function.terminator(unavailable_block).expect("a terminated block");
            self.function.insert(new_load, llrm_mir::edit::Position::Before(terminator)).expect("a placed terminator");
            // The tags and the facts the old load carries.
            for (kind, node) in &load_instruction.metadata {
                if COPIED_METADATA.contains(&kind.as_str()) {
                    self.function.annotate(new_load, kind, *node);
                }
            }
            if let Some((_, node)) = load_instruction.metadata.iter().find(|(kind, _)| kind == "access_group") {
                self.ensure_loops();
                let same_loop = {
                    let info = self.loops.as_ref().expect("made");
                    info.get_loop_for(load_block).map(|one| one.id())
                        == info.get_loop_for(unavailable_block).map(|one| one.id())
                };
                if same_loop {
                    self.function.annotate(new_load, "access_group", *node);
                }
            }
            let new_value = Operand::Value(self.function.instruction(new_load).result.expect("a load"));
            values_per_block
                .push(AvailableValueInBlock { bb: unavailable_block, av: AvailableValue::get(new_value, 0) });
            self.md_invalidate_cached_pointer_info(load_ptr);
            if let Some(&(_, old_load)) = critical_edge_pred_and_load
                .and_then(|pairs| pairs.iter().find(|(block, _)| *block == unavailable_block))
            {
                crate::local::combine_metadata_for_cse(&self.outer.metadata, self.function, new_load, old_load, true);
                let old_value = self.function.instruction(old_load).result.expect("a load");
                self.function.replace_all_uses_with(old_value, new_value);
                self.replace_values_per_block_entry(values_per_block, old_load, new_load);
                let number = self.vn.lookup(Operand::Value(old_value), false);
                if number != 0 {
                    let parent = self.function.parent(old_load).expect("a placed load");
                    self.leader_table.erase(number, Operand::Value(old_value), parent);
                }
                self.remove_instruction(old_load);
            }
        }
        // Perform PHI construction.
        let value = self.construct_ssa_for_load_set(load, values_per_block);
        let result = load_instruction.result.expect("a load has a value");
        self.function.replace_all_uses_with(result, value);
        self.md_invalidate_cached_pointer_info(value);
        self.salvage_and_remove_instruction(load);
    }

    /// `performLoadPRE`
    /// ([GVN.cpp:1789](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1789)):
    /// make `load` fully available by loading in the one predecessor that lacks
    /// it.
    pub(super) fn perform_load_pre(
        &mut self,
        load: InstId,
        values_per_block: &mut Vec<AvailableValueInBlock>,
        unavailable_blocks: &[BlockId],
    ) -> bool {
        // Okay, we have *some* definitions of the value. This means that the
        // value is available in some of our (transitive) predecessors.
        // Lets think about doing PRE of this load.
        let mut load_bb = self.function.parent(load).expect("a placed load");
        // Let's find the first basic block with more than one predecessor. Walk
        // backwards through predecessors if needed.
        let mut must_ensure_safety_of_speculative_execution = self.is_dominated_by_icf_from_same_block(load);
        let mut tmp_bb = load_bb;
        while let Some(single) = crate::edges::single_predecessor(self.function, tmp_bb) {
            tmp_bb = single;
            // Infinite (unreachable) loop.
            if tmp_bb == load_bb {
                return false;
            }
            // If some of the blockers on the way are unavailable, we can't PRE.
            if unavailable_blocks.contains(&tmp_bb) {
                return false;
            }
            // Otherwise the walk crosses only blocks with one way out.
            if self.function.successors(tmp_bb).len() != 1 {
                return false;
            }
            must_ensure_safety_of_speculative_execution =
                must_ensure_safety_of_speculative_execution || self.has_icf(tmp_bb);
        }
        load_bb = tmp_bb;
        // Check to see how many predecessors have the loaded value fully
        // available.
        let mut pred_loads: Vec<(BlockId, Option<Operand>)> = Vec::new();
        let mut fully_available_blocks: IdMap<BlockId, Availability> = IdMap::new();
        for one in values_per_block.iter() {
            fully_available_blocks.insert(one.bb, Availability::Available);
        }
        for &unavailable in unavailable_blocks {
            fully_available_blocks.insert(unavailable, Availability::Unavailable);
        }
        // The edges that need splitting, and the loads that may be moved into
        // their predecessor.
        let mut critical_edge_pred_split: Vec<BlockId> = Vec::new();
        let mut critical_edge_pred_and_load: Vec<(BlockId, InstId)> = Vec::new();
        for pred in self.function.predecessors(load_bb).iter().copied().collect::<Vec<_>>() {
            // Landing pads and the like are not handled by MIR.
            if self.is_value_fully_available_in_block(pred, &mut fully_available_blocks) {
                continue;
            }
            // If this load is not available in multiple predecessors, reject
            // it.
            let edges = self
                .function
                .terminator(pred)
                .map(|term| {
                    self.function
                        .instruction(term)
                        .operands
                        .iter()
                        .filter(|one| matches!(one, Operand::Block(_)))
                        .count()
                })
                .unwrap_or(0);
            if edges != 1 {
                // A landing pad cannot take a new predecessor.
                if self
                    .function
                    .iter_block(load_bb)
                    .find(|&one| self.function.instruction(one).opcode != Opcode::Phi)
                    .is_some_and(|first| matches!(
                        self.function.instruction(first).opcode,
                        Opcode::LandingPad { .. }
                    ))
                {
                    return false;
                }
                // A critical edge: splitting it is worth it only if it is not a
                // back edge (`enable-load-pre-split-backedge`
                // is off).
                if self.dt.dominates(load_bb, pred) {
                    return false;
                }
                match self.find_load_to_hoist_into_pred(pred, load_bb, load) {
                    Some(hoisted) => critical_edge_pred_and_load.push((pred, hoisted)),
                    None => critical_edge_pred_split.push(pred),
                }
            } else {
                // Only add the predecessors that will not be split for now.
                if !pred_loads.iter().any(|(block, _)| *block == pred) {
                    pred_loads.push((pred, None));
                }
            }
        }
        let num_insert_preds = pred_loads.len() + critical_edge_pred_split.len();
        let num_unavailable_preds = num_insert_preds + critical_edge_pred_and_load.len();
        assert!(num_unavailable_preds != 0, "Fully available value should already be eliminated!");
        // PRE is only worth it when it needs one new load, the others being
        // moved.
        if num_insert_preds > 1 {
            return false;
        }
        // Speculating the load needs it to be safe where it lands.
        if must_ensure_safety_of_speculative_execution {
            let safe = |pass: &Self| {
                crate::licm::hoist::is_safe_to_speculatively_execute_in(
                    pass.context,
                    pass.layout,
                    pass.function,
                    pass.outer,
                    load,
                )
            };
            if (!critical_edge_pred_split.is_empty()
                || !pred_loads.is_empty()
                || !critical_edge_pred_and_load.is_empty())
                && !safe(self)
            {
                return false;
            }
        }
        // Split critical edges.
        for orig_pred in critical_edge_pred_split {
            let new_pred = self.split_critical_edges(orig_pred, load_bb).expect("a critical edge splits");
            assert!(!pred_loads.iter().any(|(block, _)| *block == orig_pred), "Split edges shouldn't be in map!");
            pred_loads.push((new_pred, None));
        }
        for &(pred, _) in &critical_edge_pred_and_load {
            pred_loads.push((pred, None));
        }
        let mut can_do_pre = true;
        let mut new_insts: Vec<InstId> = Vec::new();
        let load_block = self.function.parent(load).expect("a placed load");
        for index in 0..pred_loads.len() {
            let unavailable_pred = pred_loads[index].0;
            // Do PHI translation to get its value in the predecessor if
            // necessary. The returned value may be null if no value
            // is available there.
            let mut load_ptr = Some(self.function.instruction(load).operands[0]);
            // Walk up the single-predecessor chain to the block with several.
            let mut cur = load_block;
            while cur != load_bb {
                let pred = crate::edges::single_predecessor(self.function, cur).expect("a single predecessor");
                let mut address =
                    llrm_analysis::mssa::phitrans::PhiTransAddr::new(self.function, load_ptr.expect("an address"));
                load_ptr = address.translate_with_insertion(
                    self.context,
                    self.layout,
                    self.function,
                    &self.dt,
                    cur,
                    pred,
                    &mut new_insts,
                );
                if load_ptr.is_none() {
                    can_do_pre = false;
                    break;
                }
                cur = pred;
            }
            if let Some(pointer) = load_ptr {
                let mut address = llrm_analysis::mssa::phitrans::PhiTransAddr::new(self.function, pointer);
                load_ptr = address.translate_with_insertion(
                    self.context,
                    self.layout,
                    self.function,
                    &self.dt,
                    load_bb,
                    unavailable_pred,
                    &mut new_insts,
                );
            }
            if load_ptr.is_none() {
                // The address cannot be made available in the predecessor.
                can_do_pre = false;
                break;
            }
            pred_loads[index].1 = load_ptr;
        }
        if !can_do_pre {
            // Erase the instructions made for the translation.
            while let Some(inst) = new_insts.pop() {
                self.function.erase(inst).expect("nothing uses it");
            }
            return false;
        }
        // Okay, we can eliminate this load by inserting a reload in the
        // predecessor.
        for &inst in &new_insts {
            let value = Operand::Value(self.function.instruction(inst).result.expect("a value"));
            self.lookup_or_add(value);
        }
        let available_loads: Vec<(BlockId, Operand)> =
            pred_loads.iter().map(|&(block, pointer)| (block, pointer.expect("an address"))).collect();
        self.eliminate_partially_redundant_load(
            load,
            values_per_block,
            &available_loads,
            Some(&critical_edge_pred_and_load),
        );
        true
    }

    /// `performLoopLoadPRE`
    /// ([GVN.cpp:1969](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1969)):
    /// the load at a loop header, whose address does not change in the loop and
    /// which is available from the preheader but not along one in-loop
    /// path, is loaded in both.
    pub(super) fn perform_loop_load_pre(
        &mut self,
        load: InstId,
        values_per_block: &mut Vec<AvailableValueInBlock>,
        unavailable_blocks: &[BlockId],
    ) -> bool {
        let load_block = self.function.parent(load).expect("a placed load");
        let load_ptr = self.function.instruction(load).operands[0];
        let (preheader, latch, loop_block);
        self.ensure_loops();
        {
            let info = self.loops.as_ref().expect("made");
            let Some(one) = info.get_loop_for(load_block) else { return false };
            if one.get_header() != load_block {
                return false;
            }
            // TODO: Support critical edge splitting if blocker has more than 1
            // successor.
            let (Some(pre), Some(lat)) = (one.get_loop_preheader(self.function), one.get_loop_latch(self.function))
            else {
                return false;
            };
            // Is the address invariant?
            if !one.is_loop_invariant(self.function, load_ptr) {
                return false;
            }
            (preheader, latch) = (pre, lat);
            // Check that the unavailable blocks are in the loop, and that all
            // but one is the preheader's.
            let mut found: Option<BlockId> = None;
            for &blocker in unavailable_blocks {
                if !one.contains(blocker) {
                    continue;
                }
                // Only one block of the loop.
                if found.is_some() {
                    return false;
                }
                // It must be in this loop and not in an inner one.
                if info.get_loop_for(blocker).map(|inner| inner.id()) != Some(one.id()) {
                    return false;
                }
                // It must not dominate the latch: it is not on every path.
                if self.dt.dominates(blocker, lat) {
                    return false;
                }
                // The terminator must not write memory.
                if self.terminator_may_write_to_memory(blocker) {
                    return false;
                }
                found = Some(blocker);
            }
            let Some(blocker) = found else { return false };
            loop_block = blocker;
        }
        if self.is_dominated_by_icf_from_same_block(load) {
            return false;
        }
        // The address can be freed in the loop: the preheader load may be
        // stale.
        if self.can_be_freed(load_ptr) {
            return false;
        }
        let available_loads = [(loop_block, load_ptr), (preheader, load_ptr)];
        let _ = latch;
        self.eliminate_partially_redundant_load(load, values_per_block, &available_loads, None);
        true
    }

    /// `Instruction::mayWriteToMemory` of `block`'s terminator.
    fn terminator_may_write_to_memory(
        &self,
        block: BlockId,
    ) -> bool {
        let Some(terminator) = self.function.terminator(block) else { return false };
        matches!(
            self.function.instruction(terminator).opcode,
            Opcode::Invoke(_)
        )
    }

    /// Makes the loop info of the function as it stands, when it is first
    /// asked.
    fn ensure_loops(&mut self) {
        if self.loops.is_none() {
            let function = &*self.function;
            let dt = &*self.dt;
            self.loops = Some(LoopInfo::analyze(function, || dt));
        }
    }
}
