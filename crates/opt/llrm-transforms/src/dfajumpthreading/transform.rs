//! `TransformDFA`: copy the blocks of each threading path per exit state and
//! send the path through the copies, past the `switch`.

use llrm_analysis::ssaupdaterbulk::SsaUpdaterBulk;
use llrm_mir::codemetrics::{CodeMetrics, Convergence};
use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::{Position, ValueMap};
use llrm_mir::module::{BlockId, InstId, Operand, Use};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::types::Type;

use super::paths::{PathType, SwitchPaths, ThreadingPath};
use super::{Run, incoming_value_for_block, successor_edges, switch_cases};
use crate::edges::{phis, single_predecessor, starts_with_landing_pad};
use crate::lcssa::{arms, from_arms};

/// `ClonedBlock`: a copy of a block, for a state.
struct ClonedBlock {
    bb: BlockId,
    state: u128,
}

/// `DuplicateBlockMap`: every block that has been cloned. If two threading
/// paths clone the same block for a certain state it is reused.
type DuplicateMap = IdMap<BlockId, Vec<ClonedBlock>>;

/// `DefMap`: the new definitions of an instruction, in the order they were
/// made. Needed when restoring SSA form after cloning blocks.
#[derive(Default)]
struct DefMap {
    order: Vec<InstId>,
    cloned: IdMap<InstId, Vec<InstId>>,
}

impl DefMap {
    fn push(
        &mut self,
        inst: InstId,
        copy: InstId,
    ) {
        if !self.cloned.contains_key(&inst) {
            self.order.push(inst);
        }
        self.cloned.get_or_insert_with(inst, Vec::new).push(copy);
    }
}

/// `getClonedBB`: the copy of `bb` made for `state`, if there is one.
fn cloned_bb(
    duplicates: &DuplicateMap,
    bb: BlockId,
    state: u128,
) -> Option<BlockId> {
    duplicates.get(&bb)?.iter().find(|one| one.state == state).map(|one| one.bb)
}

pub(super) struct TransformDfa<'r, 'a, 'u> {
    pub(super) run: &'r mut Run<'a, 'u>,
    pub(super) paths: SwitchPaths,
    /// The values only an `llvm.assume` reads.
    pub(super) eph: IdSet<InstId>,
}

impl TransformDfa<'_, '_, '_> {
    /// `TransformDFA::run`
    /// ([L914](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L914)).
    pub(super) fn run(mut self) -> bool {
        if self.is_legal_and_profitable_to_transform() {
            self.create_all_exit_paths();
            true
        } else {
            false
        }
    }

    /// `BB->size()`.
    fn size(
        &self,
        bb: BlockId,
    ) -> u64 {
        self.run.function().iter_block(bb).count() as u64
    }

    /// `isLegalAndProfitableToTransform`
    /// ([L928](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L928)):
    /// a legality check and a profitability check at the same time, since it is
    /// convenient: it iterates through all blocks that will be cloned, and
    /// keeps track of the duplication cost. It also returns false if it is
    /// illegal to clone some required block.
    fn is_legal_and_profitable_to_transform(&self) -> bool {
        let function = self.run.function();
        let context = &*self.run.unit.context;
        let mut metrics = CodeMetrics::default();
        let mut num_cloned_inst = 0u64;
        let switch = self.paths.switch;

        let cases = switch_cases(function, context, switch);
        // Don't thread switch without multiple successors.
        let num_successors = cases.len() as u32 + 1;
        if num_successors <= 1 {
            return false;
        }

        // DF3: the switch block is always copied, and a landing pad is not.
        if starts_with_landing_pad(function, self.paths.switch_block) {
            return false;
        }

        // Note that DuplicateBlockMap is not being used as intended here. It is
        // just being used to ensure (BB, State) pairs are only counted once.
        let mut duplicate_map = DuplicateMap::new();
        let query = self.run.cost_query();
        for tpath in &self.paths.paths {
            let path_bbs = &tpath.path;
            let next_state = tpath.exit_value;
            let determinator = tpath.determinator;

            // Update metrics for the switch block, this is always cloned.
            let bb = self.paths.switch_block;
            if cloned_bb(&duplicate_map, bb, next_state).is_none() {
                metrics.analyze_basic_block(&query, function, bb, &self.eph, false);
                num_cloned_inst += self.size(bb);
                duplicate_map.get_or_insert_with(bb, Vec::new).push(ClonedBlock { bb, state: next_state });
            }

            // If the switch block is the determinator, then we can continue
            // since this is the only block that is cloned and we already
            // counted for it.
            if path_bbs.front() == Some(&determinator) {
                continue;
            }

            // Otherwise update metrics for all blocks that will be cloned. If
            // any block is already cloned and would be reused, don't double
            // count it.
            let det_at = path_bbs.iter().position(|one| *one == determinator).unwrap_or(path_bbs.len());
            // DF3: a landing pad is not copied.
            if path_bbs.iter().skip(det_at).any(|&bb| starts_with_landing_pad(function, bb)) {
                return false;
            }
            for &bb in path_bbs.iter().skip(det_at) {
                if cloned_bb(&duplicate_map, bb, next_state).is_some() {
                    continue;
                }
                metrics.analyze_basic_block(&query, function, bb, &self.eph, false);
                num_cloned_inst += self.size(bb);
                duplicate_map.get_or_insert_with(bb, Vec::new).push(ClonedBlock { bb, state: next_state });
            }

            if metrics.not_duplicatable {
                return false;
            }
            // FIXME: Allow jump threading with controlled convergence.
            if metrics.convergence != Convergence::None {
                return false;
            }
            if !metrics.num_insts.valid {
                return false;
            }
        }

        // Too much cloned instructions slow down later optimizations,
        // especially SLPVectorizer.
        let mut num_orig_inst = 0u64;
        let mut num_outer_use_block = 0u64;
        for bb in duplicate_map.keys() {
            num_orig_inst += self.size(bb);
            // Only unduplicated blocks with single predecessor require new phi
            // nodes.
            for succ in successor_edges(function, bb) {
                if !duplicate_map.contains_key(&succ) && single_predecessor(function, succ).is_some() {
                    num_outer_use_block += 1;
                }
            }
        }

        if num_cloned_inst as f64 / num_orig_inst as f64 > self.run.options.max_cloned_rate {
            return false;
        }

        // Too much unduplicated blocks with outer uses may cause too much
        // insertions of phi nodes for duplicated definitions.
        if num_outer_use_block > u64::from(self.run.options.max_outer_use_blocks) {
            return false;
        }

        let case_values: Vec<i128> = cases.iter().map(|&(bits, _)| signed(self.switch_width(), bits)).collect();
        let (_, jump_table_size) =
            self.run.tti.estimated_number_of_case_clusters(&case_values, self.switch_width(), false);
        let duplication_cost = if jump_table_size == 0 {
            // Factor in the number of conditional branches reduced from jump
            // threading. Assume that lowering the switch block is implemented
            // by using binary search, hence the LogBase2().
            let cond_branches = i64::from(num_successors.next_power_of_two().trailing_zeros());
            metrics.num_insts.value / cond_branches
        } else {
            // Compared with jump tables, the DFA optimizer removes an indirect
            // branch on each loop iteration, thus making branch prediction
            // more precise: the more branch targets there are, the lower is
            // the cost of the DFA opt.
            metrics.num_insts.value / jump_table_size as i64
        };

        duplication_cost <= i64::from(self.run.options.cost_threshold)
    }

    /// The bit width of the switch condition.
    fn switch_width(&self) -> u32 {
        let function = self.run.function();
        let ty = function
            .operand_type(&*self.run.unit.context, function.instruction(self.paths.switch).operands[0])
            .expect("a typed condition");
        match self.run.unit.context.types.get(ty) {
            Type::Int(width) => *width,
            other => unreachable!("a switch on an integer, not {other:?}"),
        }
    }

    /// `createAllExitPaths`
    /// ([L1092](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L1092)):
    /// transform each threading path to effectively jump thread the DFA.
    fn create_all_exit_paths(&mut self) {
        // Move the switch block to the end of the path, since it will be
        // duplicated.
        let switch_block = self.paths.switch_block;
        let mut tpaths = std::mem::take(&mut self.paths.paths);
        for tpath in &mut tpaths {
            // TODO (LLVM): fix exit path creation logic so that we dont need
            // this placeholder.
            tpath.path.push_front(switch_block);
        }

        // Transform the ThreadingPaths and keep track of the cloned values.
        let mut duplicate_map = DuplicateMap::new();
        let mut new_defs = DefMap::default();

        let mut blocks_to_clean = BlocksToClean::default();
        for succ in successor_edges(self.run.function(), switch_block) {
            blocks_to_clean.insert(succ);
        }

        for tpath in &tpaths {
            self.create_exit_path(&mut new_defs, tpath, &mut duplicate_map, &mut blocks_to_clean);
        }

        // After all paths are cloned, now update the last successor of the
        // cloned path so it skips over the switch statement.
        for tpath in &tpaths {
            self.update_last_successor(tpath, &duplicate_map);
        }

        // For each instruction that was cloned and used outside, update its
        // uses.
        self.update_ssa(&new_defs);

        // Clean PHI nodes for the newly created blocks.
        for bb in blocks_to_clean.order {
            self.clean_phi_nodes(bb);
        }
    }

    /// `createExitPath`
    /// ([L1133](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L1133)):
    /// for a specific path, create an exit path starting from the determinator
    /// block. To remember the correct destination, we have to duplicate blocks
    /// corresponding to each state. Also update the terminating instruction of
    /// the predecessors, and phis in the successor blocks.
    fn create_exit_path(
        &mut self,
        new_defs: &mut DefMap,
        path: &ThreadingPath,
        duplicate_map: &mut DuplicateMap,
        blocks_to_clean: &mut BlocksToClean,
    ) {
        let next_state = path.exit_value;
        let determinator = path.determinator;
        let mut path_bbs: PathType = path.path.clone();

        // Don't select the placeholder block in front.
        if path_bbs.front() == Some(&determinator) {
            path_bbs.pop_front();
        }

        let det_at = path_bbs.iter().position(|one| *one == determinator).expect("the determinator is on its path");
        // When there is only one BB in PathBBs, the determinator takes itself
        // as a direct predecessor.
        let mut prev_bb = if path_bbs.len() == 1 { path_bbs[det_at] } else { path_bbs[det_at - 1] };
        for &bb in path_bbs.iter().skip(det_at) {
            blocks_to_clean.insert(bb);

            // We already cloned BB for this NextState, now just update the
            // branch and continue.
            if let Some(next_bb) = cloned_bb(duplicate_map, bb, next_state) {
                self.update_predecessor(prev_bb, bb, next_bb);
                prev_bb = next_bb;
                continue;
            }

            // Clone the BB and update the successor of Prev to jump to the new
            // block.
            let new_bb = self.clone_block_and_update_predecessor(bb, prev_bb, next_state, duplicate_map, new_defs);
            duplicate_map.get_or_insert_with(bb, Vec::new).push(ClonedBlock { bb: new_bb, state: next_state });
            blocks_to_clean.insert(new_bb);
            prev_bb = new_bb;
        }
    }

    /// `updateSSA`
    /// ([L1177](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L1177)):
    /// restore SSA form after cloning blocks. Each cloned block creates new
    /// defs for a variable, and the uses need to be updated to reflect this.
    /// The uses may be replaced with a cloned value, or some derived phi
    /// instruction. All uses of a value defined in the same block were already
    /// remapped when cloning the block.
    fn update_ssa(
        &mut self,
        new_defs: &DefMap,
    ) {
        let mut ssa_update = SsaUpdaterBulk::new();
        let function = self.run.function();

        for &inst in &new_defs.order {
            let cloned = new_defs.cloned.get(&inst).expect("a definition with copies");
            let bb = function.parent(inst).expect("a placed definition");
            let value = function.instruction(inst).result.expect("a cloned definition has a value");

            // Scan all uses of this instruction to see if it is used outside of
            // its block, and if so, record them to rename.
            let mut uses_to_rename: Vec<Use> = function
                .users(value)
                .iter()
                .filter(|one| {
                    let user = function.instruction(one.user);
                    if user.opcode == Opcode::Phi {
                        user.operands[one.index as usize + 1] != Operand::Block(bb)
                    } else {
                        function.parent(one.user) != Some(bb)
                    }
                })
                .copied()
                .collect();

            // If there are no uses outside the block, we're done with this
            // instruction.
            if uses_to_rename.is_empty() {
                continue;
            }

            // We found a use of I outside of BB. Rename all uses of I that are
            // outside its block to be uses of the appropriate PHI node etc.
            let name = function.value(value).name.clone();
            let var = ssa_update.add_variable(name.as_deref(), function.instruction(inst).ty);
            ssa_update.add_available_value(var, bb, Operand::Value(value));
            for &new in cloned {
                let block = function.parent(new).expect("a placed clone");
                let result = function.instruction(new).result.expect("a clone has a value");
                ssa_update.add_available_value(var, block, Operand::Value(result));
            }
            while let Some(one) = uses_to_rename.pop() {
                ssa_update.add_use(var, one);
            }
        }

        // SSAUpdater handles phi placement and renaming uses with the
        // appropriate value, over the dominator tree of the threaded function
        // (the pin keeps its own up to date: DF2).
        let unit = &mut *self.run.unit;
        let dt = DomTree::of(unit.function);
        ssa_update.rewrite_all_uses(unit.context, unit.function, &dt, None);
    }

    /// `getNextCaseSuccessor` (static)
    /// ([L1229](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L1229)):
    /// the successor of `switch` a particular case value picks.
    fn next_case_successor(
        &self,
        switch: InstId,
        next_state: u128,
    ) -> BlockId {
        let function = self.run.function();
        switch_cases(function, &*self.run.unit.context, switch)
            .into_iter()
            .find(|&(bits, _)| bits == next_state)
            .map_or_else(|| super::default_dest(function, switch), |(_, to)| to)
    }

    /// `cloneBlockAndUpdatePredecessor`
    /// ([L1247](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L1247)):
    /// clones a basic block and adds it to the CFG; also updates the phi nodes
    /// in the successors of the BB, and remaps uses that were defined locally
    /// in the cloned BB.
    fn clone_block_and_update_predecessor(
        &mut self,
        bb: BlockId,
        prev_bb: BlockId,
        next_state: u128,
        duplicate_map: &DuplicateMap,
        new_defs: &mut DefMap,
    ) -> BlockId {
        let suffix = format!(".jt{}", u64::try_from(next_state).unwrap_or(u64::MAX));
        let function = &mut *self.run.unit.function;
        let mut map = ValueMap::default();
        let new_bb = function.clone_block(bb, &mut map);
        if let Some(name) = function.block(bb).name.clone() {
            function.set_block_name(new_bb, &format!("{name}{suffix}"));
        }
        // moveAfter(BB)
        function.insert_block(new_bb, Some(bb)).expect("a new block");

        // The pairs (original, copy) of the instructions that have a value, in
        // the order of the block.
        let originals: Vec<InstId> = function.iter_block(bb).collect();
        let copies: Vec<InstId> = function.iter_block(new_bb).collect();
        let mut pairs: Vec<(InstId, InstId)> = Vec::new();
        for (&from, &to) in originals.iter().zip(&copies) {
            let (Some(old_value), Some(new_value)) =
                (function.instruction(from).result, function.instruction(to).result)
            else {
                continue;
            };
            if let Some(name) = function.value(old_value).name.clone() {
                function.set_name(new_value, &format!("{name}{suffix}"));
            }
            pairs.push((from, to));
        }

        for &inst in &copies {
            // Do not remap operands of PHINode in case a definition in BB is an
            // incoming value to a phi in the same block. This incoming value
            // will be renamed later while restoring SSA.
            if function.instruction(inst).opcode == Opcode::Phi {
                continue;
            }
            function.remap_instruction(inst, &map);
        }

        self.update_successor_phis(bb, new_bb, next_state, &map, duplicate_map);
        self.update_predecessor(prev_bb, bb, new_bb);

        // `updateDefMap`: the new definitions of each instruction, to restore
        // SSA form from.
        for (from, to) in pairs {
            new_defs.push(from, to);
        }
        new_bb
    }

    /// `updateSuccessorPhis`
    /// ([L1295](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L1295)):
    /// update the phi nodes in BB's successors: a new incoming value from
    /// `cloned_bb` with the new instruction wherever there is an incoming value
    /// from BB.
    fn update_successor_phis(
        &mut self,
        bb: BlockId,
        cloned_block: BlockId,
        next_state: u128,
        map: &ValueMap,
        duplicate_map: &DuplicateMap,
    ) {
        let mut blocks_to_update: Vec<BlockId> = Vec::new();

        if bb == self.paths.switch_block {
            // If BB is the last block in the path, we can simply update the one
            // case successor that will be reached.
            let next_case = self.next_case_successor(self.paths.switch, next_state);
            blocks_to_update.push(next_case);
            if let Some(cloned_succ) = cloned_bb(duplicate_map, next_case, next_state) {
                blocks_to_update.push(cloned_succ);
            }
        } else {
            // Otherwise update phis in all successors.
            for succ in successor_edges(self.run.function(), bb) {
                blocks_to_update.push(succ);

                // Check if a successor has already been cloned for the
                // particular exit value. In this case if a successor was
                // already cloned, the phi nodes in the cloned block should be
                // updated directly.
                if let Some(cloned_succ) = cloned_bb(duplicate_map, succ, next_state) {
                    blocks_to_update.push(cloned_succ);
                }
            }
        }

        // If there is a phi with an incoming value from BB, create a new
        // incoming value for the new predecessor ClonedBB. The value will
        // either be the same value from BB or a cloned value.
        for succ in blocks_to_update {
            for phi in phis(self.run.function(), succ) {
                let Some(incoming) = incoming_value_for_block(self.run.function(), phi, bb) else { continue };
                let value = match incoming {
                    Operand::Value(id) => map.values.get(&id).copied().unwrap_or(incoming),
                    other => other,
                };
                let mut operands = self.run.function().instruction(phi).operands.clone();
                operands.extend([value, Operand::Block(cloned_block)]);
                self.run.unit.function.set_operands(phi, operands);
            }
        }
    }

    /// `updatePredecessor`
    /// ([L1347](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L1347)):
    /// sets the successor of `prev_bb` to be `new_bb` instead of `old_bb`; all
    /// other successors are kept as well.
    fn update_predecessor(
        &mut self,
        prev_bb: BlockId,
        old_bb: BlockId,
        new_bb: BlockId,
    ) {
        // When a path is reused, there is a chance that predecessors were
        // already updated before. Check if the predecessor needs to be updated
        // first.
        if !self.is_predecessor(old_bb, prev_bb) {
            return;
        }

        let term = self.run.function().terminator(prev_bb).expect("a terminated block");
        let count = self.run.function().instruction(term).operands.len();
        for index in 0..count {
            if self.run.function().instruction(term).operands[index] == Operand::Block(old_bb) {
                let unit = &mut *self.run.unit;
                unit.function.predecessor_removed(unit.context, old_bb, prev_bb, true);
                unit.function.set_operand(term, index, Operand::Block(new_bb));
            }
        }
    }

    /// `updateLastSuccessor`
    /// ([L1401](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L1401)):
    /// update the last branch of a particular cloned path to point to the
    /// correct case successor. An optional step, but it makes the CFG
    /// significantly easier to work with.
    fn update_last_successor(
        &mut self,
        tpath: &ThreadingPath,
        duplicate_map: &DuplicateMap,
    ) {
        let next_state = tpath.exit_value;
        let bb = *tpath.path.back().expect("a path has blocks");
        let last_block = cloned_bb(duplicate_map, bb, next_state).expect("the last block of a path is cloned");

        // Note multiple paths can end at the same block so check that it is not
        // updated yet.
        let term = self.run.function().terminator(last_block).expect("a terminated block");
        if self.run.function().instruction(term).opcode != Opcode::Switch {
            return;
        }
        let next_case = self.next_case_successor(term, next_state);

        let void = self.run.function().instruction(term).ty;
        let function = &mut *self.run.unit.function;
        function.erase(term).expect("a switch nothing uses");
        let jump =
            function.create_instruction(Opcode::Br, void, vec![Operand::Block(next_case)], Flags::default(), None);
        function.insert(jump, Position::End(last_block)).expect("a placed block");
    }

    /// `cleanPhiNodes`
    /// ([L1431](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L1431)):
    /// after cloning blocks, some of the phi nodes have extra incoming values
    /// that are no longer used. This removes them.
    fn clean_phi_nodes(
        &mut self,
        bb: BlockId,
    ) {
        let predecessors = self.run.function().predecessors(bb);
        let phis = phis(self.run.function(), bb);
        // If BB is no longer reachable, remove any remaining phi nodes.
        if predecessors.is_empty() {
            for phi in phis {
                self.replace_with_poison_and_erase(phi);
            }
            return;
        }

        // Remove any incoming values that come from an invalid predecessor.
        for phi in phis {
            let incoming = arms(self.run.function(), phi);
            let kept: Vec<_> = incoming.iter().copied().filter(|(_, from)| predecessors.contains(from)).collect();
            if kept.len() == incoming.len() {
                continue;
            }
            if kept.is_empty() {
                self.replace_with_poison_and_erase(phi);
            } else {
                self.run.unit.function.set_operands(phi, from_arms(&kept));
            }
        }
    }

    /// `PN.replaceAllUsesWith(PoisonValue::get(PN.getType()));
    /// PN.eraseFromParent()`.
    fn replace_with_poison_and_erase(
        &mut self,
        phi: InstId,
    ) {
        let unit = &mut *self.run.unit;
        let ty = unit.function.instruction(phi).ty;
        let poison = Operand::Constant(unit.context.constant(Constant { ty, kind: ConstantKind::Poison }));
        let value = unit.function.instruction(phi).result.expect("a phi has a value");
        unit.function.replace_value(value, poison);
        unit.function.set_operands(phi, Vec::new());
        unit.function.erase(phi).expect("a phi nothing reads");
    }

    /// `isPredecessor`: whether `incoming` is a predecessor of `bb`.
    fn is_predecessor(
        &self,
        bb: BlockId,
        incoming: BlockId,
    ) -> bool {
        self.run.function().predecessors(bb).contains(&incoming)
    }
}

/// `SmallSetVector<BasicBlock *, 16>`: blocks once each, in the order they were
/// first inserted.
#[derive(Default)]
struct BlocksToClean {
    order: Vec<BlockId>,
    seen: IdSet<BlockId>,
}

impl BlocksToClean {
    fn insert(
        &mut self,
        bb: BlockId,
    ) {
        if self.seen.insert(bb) {
            self.order.push(bb);
        }
    }
}

/// The value of `bits` as a `width` bits signed integer.
fn signed(
    width: u32,
    bits: u128,
) -> i128 {
    if width >= 128 {
        return bits as i128;
    }
    let shift = 128 - width;
    ((bits << shift) as i128) >> shift
}
