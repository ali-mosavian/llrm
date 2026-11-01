//! `RAGreedy::trySplit` and what it calls: the region, block and local splits.
//! Translated from llvm/lib/CodeGen/RegAllocGreedy.cpp and RegAllocGreedy.h at
//! d1106deb71cc.
//!
//! # How it works
//! A range that does not fit anywhere is split before it is spilled. `trySplit`
//! sends a range that lives in one block to `tryLocalSplit`, which finds the
//! stretch of its uses that can take a register and cuts the rest away.
//! Otherwise `tryRegionSplit` asks, for each register of the allocation order,
//! which edge bundles should hold the value in that register
//! ([`SpillPlacement`]) given where the register is taken
//! ([`InterferenceCache`]), costs the result against isolating every block, and
//! splits around the cheapest ([`SplitEditor`]); `tryBlockSplit` isolates the
//! blocks that use the value.
//!
//! ```text
//!   range      |--use--|  ....  |--use--|
//!   region     [=======reg======]           (bundles that hold the register)
//!   result     c1=copy v ... use ... c0=copy c1     the rest stays on the stack
//! ```
//!
//! DEVIATE D-RS1: `RegClassInfo.isProperSubClass` is false unless the target
//! says a value's order has a larger one (`TargetInstrInfo::larger_order`), so
//! `SingleInstrs` is false for the shadow allocator's roots and
//! `tryInstructionSplit` returns at once; a value is one class here (no
//! subranges).
//!
//! DEVIATE D-RS2: no `IMPLICIT_DEF`: `addSplitConstraints`' test of the last
//! instruction for an implicit def is dropped, LIR has none.
//!
//! DEVIATE D-RS3: no `trySplitAroundHintReg` (as in `greedy`);
//! `shouldRegionSplitForVirtReg` is true (it only declines a huge
//! rematerializable value, and rematerialization is recognised per definition,
//! `greedy/remat.rs`).
//!
//! DEVIATE D-RS4: `DebugVars->splitRegister` and the verifier calls are not
//! here: LIR has neither.
//!
//! # Example
//! A value live through a loop that uses it twice, with one register free in
//! the loop: the region is the loop's bundles; the value is copied into a new
//! register on entry to the loop and back on exit, and the loop reads the
//! register.
//!
//! # Used by
//! [`super::greedy::RAGreedy`] (`selectOrSplitImpl`).

use std::rc::Rc;

use llrm_lir::registers::RegId;
use llrm_mir::blockfrequency::frequency::BlockFrequency;

use super::edgebundles::EdgeBundles;
use super::greedy::{AllocationOrder, ExtraRegInfo, GreedyEnv, LiveRangeStage, is_unused_callee_saved_reg};
use super::interferencecache::{Cursor, Env, InterferenceCache};
use super::liverangeedit::{LiveRangeEdit, NoDelegate};
use super::matrix::LiveRegMatrix;
use super::spillplacement::{BlockConstraint, BorderConstraint, SpillPlacement};
use super::spliteditor::{ComplementSpillMode, SplitCx, SplitEditor};
use super::splitkit::{SplitAnalysis, SplitEnv, first_insert_instr};
use crate::liveness::blockfrequency::MachineBlockFrequencyInfo;
use crate::liveness::calcspillweights::{Hint, VirtRegAuxInfo, normalize_spill_weight};
use crate::liveness::liveintervals::LiveIntervals;
use crate::liveness::liverange::LiveInterval;
use crate::liveness::slotindex::{InstrKey, SlotIndex};
use crate::support::hash::HashMap;

/// `NoCand`.
const NO_CAND: usize = usize::MAX;

/// `Hysteresis`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L331>).
const HYSTERESIS: f32 = 2007.0 / 2048.0;

/// `-grow-region-complexity-budget`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L123>).
const GROW_REGION_COMPLEXITY_BUDGET: usize = 10000;

/// `-split-spill-mode`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L86>).
const SPLIT_SPILL_MODE: ComplementSpillMode = ComplementSpillMode::Speed;

/// `GlobalSplitCandidate`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.h#L227>).
#[derive(Default)]
pub struct GlobalSplitCandidate {
    /// Register intended for assignment, or none.
    pub phys_reg: Option<RegId>,
    /// SplitKit interval index for this candidate.
    pub intv_idx: usize,
    /// Interference for `phys_reg`.
    pub intf: Cursor,
    /// Bundles where this candidate should be live, by bundle.
    pub live_bundles: Vec<bool>,
    pub active_blocks: Vec<u32>,
}

impl GlobalSplitCandidate {
    fn reset(
        &mut self,
        cache: &mut InterferenceCache,
        env: &Env,
        reg: Option<RegId>,
    ) {
        self.phys_reg = reg;
        self.intv_idx = 0;
        self.intf.set_phys_reg(cache, env, reg);
        self.live_bundles.clear();
        self.active_blocks.clear();
    }

    /// `getBundles`: set `B[I] = C` for every live bundle where `B[I]` was
    /// `NoCand`.
    fn get_bundles(
        &self,
        b: &mut [usize],
        c: usize,
    ) -> u32 {
        let mut count = 0;
        for (i, &live) in self.live_bundles.iter().enumerate() {
            if live && b[i] == NO_CAND {
                b[i] = c;
                count += 1;
            }
        }
        count
    }

    fn live_count(&self) -> usize {
        self.live_bundles.iter().filter(|&&l| l).count()
    }
}

/// What the splitting analyses keep for a function: built once from its CFG,
/// frequencies and loops.
pub struct SplitSupport<'a> {
    pub env: Box<dyn SplitEnv + 'a>,
    pub mbfi: MachineBlockFrequencyInfo,
    pub bundles: Rc<EdgeBundles>,
    pub placer: SpillPlacement,
    pub cache: InterferenceCache,
    pub sa: SplitAnalysis,
    /// `GlobalCand`.
    pub global_cand: Vec<GlobalSplitCandidate>,
    /// `BundleCand`.
    bundle_cand: Vec<usize>,
    /// `SplitConstraints`.
    split_constraints: Vec<BlockConstraint>,
}

impl<'a> SplitSupport<'a> {
    /// `succs[n]` are the successors of block `n`.
    pub fn new(
        env: Box<dyn SplitEnv + 'a>,
        mbfi: MachineBlockFrequencyInfo,
        succs: &[Vec<usize>],
    ) -> Self {
        let n = succs.len();
        let bundles = Rc::new(EdgeBundles::new(succs));
        let freqs = (0..n).map(|b| mbfi.get_block_freq(b)).collect();
        let placer = SpillPlacement::new(Rc::clone(&bundles), mbfi.get_entry_freq(), freqs);
        Self {
            env,
            mbfi,
            bundles,
            placer,
            cache: InterferenceCache::new(n),
            sa: SplitAnalysis::new(n),
            global_cand: Vec::new(),
            bundle_cand: Vec::new(),
            split_constraints: Vec::new(),
        }
    }
}

/// The parts of the allocator the splits work on, borrowed apart so the editor
/// can hold the live intervals while the extra info is read.
pub struct Splitter<'p, 'a> {
    pub lis: &'p mut LiveIntervals,
    pub matrix: &'p mut LiveRegMatrix,
    pub extra: &'p mut ExtraRegInfo,
    pub ss: &'p mut SplitSupport<'a>,
    pub env: &'p dyn GreedyEnv,
    pub original: &'p HashMap<u32, u32>,
    pub hints: &'p mut std::collections::BTreeMap<u32, Vec<Hint>>,
}

impl Splitter<'_, '_> {
    /// `trySplit`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1975>):
    /// try to split `virt`, making it assignable. Returns a register when it
    /// may be assigned; `new_vregs` gets the ranges the split made.
    pub fn try_split(
        &mut self,
        virt: &Rc<LiveInterval>,
        order: &AllocationOrder,
        new_vregs: &mut Vec<u32>,
    ) -> Option<RegId> {
        self.matrix.fixed().set_lane(Some(virt.reg()));
        let phys = self.try_split_for(virt, order, new_vregs);
        self.matrix.fixed().set_lane(None);
        phys
    }

    fn try_split_for(
        &mut self,
        virt: &Rc<LiveInterval>,
        order: &AllocationOrder,
        new_vregs: &mut Vec<u32>,
    ) -> Option<RegId> {
        // Ranges must be Split2 or less.
        if self.extra.get_stage(virt.reg()) >= LiveRangeStage::Spill {
            return None;
        }

        // Local intervals are handled separately.
        if self.lis.interval_is_in_one_mbb(virt).is_some() {
            self.analyze(virt);
            let phys = self.try_local_split(virt, order, new_vregs);
            if phys.is_some() || !new_vregs.is_empty() {
                return phys;
            }
            return self.try_instruction_split(virt, new_vregs);
        }

        self.analyze(virt);

        // First try to split around a region spanning multiple blocks.
        // RS_Split2 ranges already made dubious progress with region
        // splitting, so they go straight to single block splitting.
        if self.extra.get_stage(virt.reg()) < LiveRangeStage::Split2 {
            let phys = self.try_region_split(virt, order, new_vregs);
            if phys.is_some() || !new_vregs.is_empty() {
                return phys;
            }
        }

        // Then isolate blocks.
        self.try_block_split(virt, new_vregs)
    }

    fn analyze(
        &mut self,
        virt: &Rc<LiveInterval>,
    ) {
        self.ss.sa.analyze(self.lis, &*self.ss.env, Rc::clone(virt));
    }

    /// `Cand.Intf.moveToBlock(num)`.
    fn intf_move(
        &mut self,
        cand: usize,
        num: u32,
    ) {
        let env = Env { matrix: &*self.matrix, indexes: &self.lis.indexes };
        self.ss.global_cand[cand].intf.move_to_block(&mut self.ss.cache, &env, num as usize);
    }

    fn block_freq(
        &self,
        num: u32,
    ) -> BlockFrequency {
        self.ss.placer.get_block_frequency(num)
    }

    /// `addSplitConstraints`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L739>):
    /// the cost of the interference pattern in the use blocks; false if no
    /// bundle prefers a register or a spill cannot be placed.
    fn add_split_constraints(
        &mut self,
        cand: usize,
        cost: &mut BlockFrequency,
    ) -> bool {
        let use_blocks = self.ss.sa.get_use_blocks().to_vec();

        // Reset interference dependent info.
        self.ss.split_constraints.clear();
        let mut static_cost = BlockFrequency(0);
        for bi in &use_blocks {
            let number = bi.mbb as u32;
            self.intf_move(cand, number);
            let mut bc = BlockConstraint {
                number,
                entry: if bi.live_in { BorderConstraint::PrefReg } else { BorderConstraint::DontCare },
                // D-RS2: no implicit def at the last instruction.
                exit: if bi.live_out { BorderConstraint::PrefReg } else { BorderConstraint::DontCare },
                changes_value: bi.first_def.is_valid(),
            };
            let intf = &self.ss.global_cand[cand].intf;
            if !intf.has_interference() {
                self.ss.split_constraints.push(bc);
                continue;
            }
            let (intf_first, intf_last) = (intf.first(), intf.last());

            // Number of spill code instructions to insert.
            let mut ins = 0u32;

            // Interference for the live-in value.
            if bi.live_in {
                if intf_first <= self.lis.indexes.get_mbb_start_idx(bi.mbb) {
                    bc.entry = BorderConstraint::MustSpill;
                    ins += 1;
                } else if intf_first < bi.first_instr {
                    bc.entry = BorderConstraint::PrefSpill;
                    ins += 1;
                } else if intf_first < bi.last_instr {
                    ins += 1;
                }

                // Abort if the spill cannot be inserted at the MBB' start.
                if matches!(
                    bc.entry,
                    BorderConstraint::MustSpill | BorderConstraint::PrefSpill
                )
                    && SlotIndex::is_earlier_instr(&bi.first_instr, &self.ss.sa.get_first_split_point(self.lis, bi.mbb))
                {
                    return false;
                }
            }

            // Interference for the live-out value.
            if bi.live_out {
                let lsp = self.ss.sa.get_last_split_point(self.lis, &*self.ss.env, bi.mbb);
                if intf_last >= lsp {
                    bc.exit = BorderConstraint::MustSpill;
                    ins += 1;
                } else if intf_last > bi.last_instr {
                    bc.exit = BorderConstraint::PrefSpill;
                    ins += 1;
                } else if intf_last > bi.first_instr {
                    ins += 1;
                }
            }

            // Accumulate the total frequency of inserted spill code.
            for _ in 0..ins {
                static_cost = static_cost + self.block_freq(number);
            }
            self.ss.split_constraints.push(bc);
        }
        *cost = static_cost;

        // Add constraints for use-blocks. Note that these are the only
        // constraints that may add a positive bias, it is downhill from here.
        self.ss.placer.add_constraints(&self.ss.split_constraints);
        self.ss.placer.scan_active_bundles()
    }

    /// `addThroughConstraints`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L812>):
    /// add constraints and links to the placer from the live-through blocks.
    fn add_through_constraints(
        &mut self,
        cand: usize,
        blocks: &[u32],
    ) -> bool {
        const GROUP_SIZE: usize = 8;
        let mut bcs: Vec<BlockConstraint> = Vec::with_capacity(GROUP_SIZE);
        let mut tbs: Vec<u32> = Vec::with_capacity(GROUP_SIZE);

        for &number in blocks {
            self.intf_move(cand, number);
            let intf = &self.ss.global_cand[cand].intf;

            if !intf.has_interference() {
                tbs.push(number);
                if tbs.len() == GROUP_SIZE {
                    self.ss.placer.add_links(&tbs);
                    tbs.clear();
                }
                continue;
            }
            let (intf_first, intf_last) = (intf.first(), intf.last());
            let mbb = number as usize;

            // Abort if the spill cannot be inserted at the MBB' start.
            if let Some(first) = self.lis.block_insns(mbb).first() {
                let first_idx = self.lis.indexes.get_instruction_index(first.id());
                if SlotIndex::is_earlier_instr(&first_idx, &self.ss.sa.get_first_split_point(self.lis, mbb)) {
                    return false;
                }
            }

            // Interference for the live-in value.
            let insert_idx = match first_insert_instr(self.lis, mbb) {
                Some(k) => self.lis.indexes.get_instruction_index(k),
                None => self.lis.indexes.get_mbb_end_idx(mbb),
            };
            let entry = if intf_first <= self.lis.indexes.get_mbb_start_idx(mbb)
                || SlotIndex::is_earlier_instr(&intf_first, &insert_idx)
            {
                BorderConstraint::MustSpill
            } else {
                BorderConstraint::PrefSpill
            };

            // Interference for the live-out value.
            let exit = if intf_last >= self.ss.sa.get_last_split_point(self.lis, &*self.ss.env, mbb) {
                BorderConstraint::MustSpill
            } else {
                BorderConstraint::PrefSpill
            };
            bcs.push(BlockConstraint { number, entry, exit, changes_value: false });

            if bcs.len() == GROUP_SIZE {
                self.ss.placer.add_constraints(&bcs);
                bcs.clear();
            }
        }

        self.ss.placer.add_constraints(&bcs);
        self.ss.placer.add_links(&tbs);
        true
    }

    /// `growRegion`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L872>):
    /// add the through blocks the positive bundles reach, until stable.
    fn grow_region(
        &mut self,
        cand: usize,
    ) -> bool {
        // Keep track of through blocks that have not been added to the placer.
        let mut todo: Vec<bool> = self.ss.sa.get_through_blocks().to_vec();
        let mut active_blocks: Vec<u32> = std::mem::take(&mut self.ss.global_cand[cand].active_blocks);
        let mut added_to = 0usize;

        let mut budget = GROW_REGION_COMPLEXITY_BUDGET;
        let ok = loop {
            let new_bundles: Vec<u32> = self.ss.placer.get_recent_positive().to_vec();
            // Find new through blocks in the periphery of PrefRegBundles.
            let mut over = false;
            for bundle in new_bundles {
                // Look at all blocks connected to Bundle in the full graph.
                let blocks = self.ss.bundles.get_blocks(bundle);
                // Limit compilation time by bailing out after we use all our
                // budget.
                if blocks.len() >= budget {
                    over = true;
                    break;
                }
                budget -= blocks.len();
                for &block in blocks {
                    if !todo[block as usize] {
                        continue;
                    }
                    todo[block as usize] = false;
                    // This is a new through block. Add it to the placer later.
                    active_blocks.push(block);
                }
            }
            if over {
                break false;
            }
            // Any new blocks to add?
            if active_blocks.len() == added_to {
                break true;
            }

            // Compute through constraints from the interference, or assume
            // that all through blocks prefer spilling when forming compact
            // regions.
            let new_blocks: Vec<u32> = active_blocks[added_to..].to_vec();
            if self.ss.global_cand[cand].phys_reg.is_some() {
                if !self.add_through_constraints(cand, &new_blocks) {
                    break false;
                }
            } else {
                // Providing that the variable being spilled does not look like
                // a loop induction variable, which is expensive to spill
                // around and better pushed into a condition inside the loop if
                // possible, provide a strong negative bias on through blocks
                // to prevent unwanted liveness on loop backedges.
                let mut pref_spill = true;
                if self.ss.sa.looks_like_loop_iv() && new_blocks.len() >= 2 {
                    // Check that the current bundle is adding a Header +
                    // start+end of loop-internal blocks. If the block is
                    // indeed a header, don't make the NewBlocks as PrefSpill to
                    // allow the variable to be live in Header<->Latch.
                    let env = &*self.ss.env;
                    let l = env.loop_of(new_blocks[0] as usize);
                    if let Some(l) = l {
                        if env.loop_header(l) == new_blocks[0] as usize
                            && new_blocks[1..].iter().all(|&b| env.loop_of(b as usize) == Some(l))
                        {
                            pref_spill = false;
                        }
                    }
                }
                if pref_spill {
                    self.ss.placer.add_pref_spill(&new_blocks, true);
                }
            }
            added_to = active_blocks.len();

            // Perhaps iterating can enable more bundles?
            self.ss.placer.iterate();
        };
        self.ss.global_cand[cand].active_blocks = active_blocks;
        ok
    }

    /// Copy the placer's answer into the candidate (`prepare(Cand.LiveBundles)`
    /// then `finish()` in C++, D-SP1).
    fn finish_placer(
        &mut self,
        cand: usize,
    ) {
        self.ss.placer.finish();
        let n = self.ss.bundles.get_num_bundles() as usize;
        let c = &mut self.ss.global_cand[cand];
        c.live_bundles.clear();
        c.live_bundles.resize(n, false);
        for b in self.ss.placer.active_bundles() {
            c.live_bundles[b as usize] = true;
        }
    }

    /// `calcCompactRegion`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L951>):
    /// the edge bundles that should be live when splitting the range into
    /// compact regions, which need no look at interference.
    fn calc_compact_region(&mut self) -> bool {
        // Without any through blocks, the live range is already compact.
        if self.ss.sa.get_num_through_blocks() == 0 {
            return false;
        }
        if self.ss.global_cand.is_empty() {
            self.ss.global_cand.push(GlobalSplitCandidate::default());
        }

        // Compact regions don't correspond to any physreg.
        {
            let env = Env { matrix: &*self.matrix, indexes: &self.lis.indexes };
            self.ss.global_cand[0].reset(&mut self.ss.cache, &env, None);
        }

        // Use the spill placer to determine the live bundles. growRegion
        // pretends that all the through blocks have interference when PhysReg
        // is unset.
        self.ss.placer.prepare();

        // The static split cost will be zero since Cand.Intf reports no
        // interference.
        let mut cost = BlockFrequency(0);
        if !self.add_split_constraints(0, &mut cost) {
            return false;
        }
        if !self.grow_region(0) {
            return false;
        }
        self.finish_placer(0);
        self.ss.global_cand[0].live_count() != 0
    }

    /// `calcBlockSplitCost`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L994>):
    /// how expensive it would be to split around all use blocks instead of
    /// forming bundle regions.
    fn calc_block_split_cost(&self) -> BlockFrequency {
        let mut cost = BlockFrequency(0);
        for bi in self.ss.sa.get_use_blocks() {
            let number = bi.mbb as u32;
            // We normally only need one spill instruction - a load or a store.
            cost = cost + self.block_freq(number);
            // Unless the value is redefined in the block.
            if bi.live_in && bi.live_out && bi.first_def.is_valid() {
                cost = cost + self.block_freq(number);
            }
        }
        cost
    }

    /// `calcGlobalSplitCost`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1013>):
    /// the global split cost of following the split pattern in `LiveBundles`.
    fn calc_global_split_cost(
        &mut self,
        cand: usize,
    ) -> BlockFrequency {
        let mut global_cost = BlockFrequency(0);
        let use_blocks = self.ss.sa.get_use_blocks().to_vec();
        for (i, bi) in use_blocks.iter().enumerate() {
            let bc = self.ss.split_constraints[i];
            let live = &self.ss.global_cand[cand].live_bundles;
            let reg_in = live[self.ss.bundles.get_bundle(bc.number, false) as usize];
            let reg_out = live[self.ss.bundles.get_bundle(bc.number, true) as usize];
            let mut ins = 0u32;

            self.intf_move(cand, bc.number);

            if bi.live_in {
                ins += (reg_in != (bc.entry == BorderConstraint::PrefReg)) as u32;
            }
            if bi.live_out {
                ins += (reg_out != (bc.exit == BorderConstraint::PrefReg)) as u32;
            }
            for _ in 0..ins {
                global_cost = global_cost + self.block_freq(bc.number);
            }
        }

        let active = self.ss.global_cand[cand].active_blocks.clone();
        for number in active {
            let live = &self.ss.global_cand[cand].live_bundles;
            let reg_in = live[self.ss.bundles.get_bundle(number, false) as usize];
            let reg_out = live[self.ss.bundles.get_bundle(number, true) as usize];
            if !reg_in && !reg_out {
                continue;
            }
            if reg_in && reg_out {
                // We need double spill code if this block has interference.
                self.intf_move(cand, number);
                if self.ss.global_cand[cand].intf.has_interference() {
                    global_cost = global_cost + self.block_freq(number);
                    global_cost = global_cost + self.block_freq(number);
                }
                continue;
            }
            // live-in / stack-out or stack-in live-out.
            global_cost = global_cost + self.block_freq(number);
        }
        global_cost
    }

    /// `splitAroundRegion`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1067>):
    /// split the current live range around the regions determined by the
    /// candidates; `se` has the intervals of the used candidates open.
    fn split_around_region(
        &mut self,
        se: &mut SplitEditor,
        used_cands: &[usize],
        new_vregs: &mut Vec<u32>,
    ) {
        // These are the intervals created for new global ranges. We may create
        // more intervals for local ranges.
        let num_global_intvs = se.edit().size();
        assert!(num_global_intvs != 0, "No global intervals configured");

        // Isolate even single instructions when dealing with a proper
        // sub-class.
        let single_instrs = self.env.is_proper_subclass(se.edit().parent());

        // First handle all the blocks with uses.
        let use_blocks = self.ss.sa.get_use_blocks().to_vec();
        for bi in &use_blocks {
            let number = bi.mbb as u32;
            let (mut intv_in, mut intv_out) = (0usize, 0usize);
            let (mut intf_in, mut intf_out) = (SlotIndex::default(), SlotIndex::default());
            if bi.live_in {
                let cand_in = self.ss.bundle_cand[self.ss.bundles.get_bundle(number, false) as usize];
                if cand_in != NO_CAND {
                    intv_in = self.ss.global_cand[cand_in].intv_idx;
                    self.intf_move(cand_in, number);
                    intf_in = self.ss.global_cand[cand_in].intf.first();
                }
            }
            if bi.live_out {
                let cand_out = self.ss.bundle_cand[self.ss.bundles.get_bundle(number, true) as usize];
                if cand_out != NO_CAND {
                    intv_out = self.ss.global_cand[cand_out].intv_idx;
                    self.intf_move(cand_out, number);
                    intf_out = self.ss.global_cand[cand_out].intf.last();
                }
            }

            let mut cx = self.cx();

            // Create separate intervals for isolated blocks with multiple
            // uses.
            if intv_in == 0 && intv_out == 0 {
                if cx.sa.should_split_single_block(cx.lis, cx.vrm, cx.env, bi, single_instrs) {
                    se.split_single_block(&mut cx, bi);
                }
                continue;
            }

            if intv_in != 0 && intv_out != 0 {
                se.split_live_through_block(&mut cx, bi.mbb, intv_in, intf_in, intv_out, intf_out);
            } else if intv_in != 0 {
                se.split_reg_in_block(&mut cx, bi, intv_in, intf_in);
            } else {
                se.split_reg_out_block(&mut cx, bi, intv_out, intf_out);
            }
        }

        // Handle live-through blocks. The relevant live-through blocks are
        // stored in the active blocks list with each candidate. We need to
        // filter out duplicates.
        let mut todo: Vec<bool> = self.ss.sa.get_through_blocks().to_vec();
        for &used in used_cands {
            let blocks = self.ss.global_cand[used].active_blocks.clone();
            for number in blocks {
                if !todo[number as usize] {
                    continue;
                }
                todo[number as usize] = false;

                let (mut intv_in, mut intv_out) = (0usize, 0usize);
                let (mut intf_in, mut intf_out) = (SlotIndex::default(), SlotIndex::default());

                let cand_in = self.ss.bundle_cand[self.ss.bundles.get_bundle(number, false) as usize];
                if cand_in != NO_CAND {
                    intv_in = self.ss.global_cand[cand_in].intv_idx;
                    self.intf_move(cand_in, number);
                    intf_in = self.ss.global_cand[cand_in].intf.first();
                }

                let cand_out = self.ss.bundle_cand[self.ss.bundles.get_bundle(number, true) as usize];
                if cand_out != NO_CAND {
                    intv_out = self.ss.global_cand[cand_out].intv_idx;
                    self.intf_move(cand_out, number);
                    intf_out = self.ss.global_cand[cand_out].intf.last();
                }
                if intv_in == 0 && intv_out == 0 {
                    continue;
                }
                let mut cx = self.cx();
                se.split_live_through_block(&mut cx, number as usize, intv_in, intf_in, intv_out, intf_out);
            }
        }

        let orig_blocks = self.ss.sa.get_num_live_blocks();

        let intv_map = self.finish_editor(se);
        self.collect_new_regs(se, new_vregs);

        // Sort out the new intervals created by splitting. We get four kinds:
        // - Remainder intervals should not be split again.
        // - Candidate intervals can be assigned to Cand.PhysReg.
        // - Block-local splits are candidates for local splitting.
        // - DCE leftovers should go back on the queue.
        for i in 0..se.edit().size() {
            let reg = se.edit().get(i);

            // Ignore old intervals from DCE.
            if self.extra.get_or_init_stage(reg) != LiveRangeStage::New {
                continue;
            }

            // Remainder interval. Don't try splitting again, spill if it
            // doesn't allocate.
            if intv_map[i] == 0 {
                self.extra.set_stage(reg, LiveRangeStage::Spill);
                continue;
            }

            // Global intervals. Allow repeated splitting as long as the number
            // of live blocks is strictly decreasing.
            if intv_map[i] < num_global_intvs {
                let li = self.lis.get_interval_rc(reg);
                if self.ss.sa.count_live_blocks(self.lis, &li) >= orig_blocks {
                    // Don't allow repeated splitting as a safe guard against
                    // looping.
                    self.extra.set_stage(reg, LiveRangeStage::Split2);
                }
                continue;
            }

            // Other intervals are treated as new. This includes local
            // intervals created for blocks with multiple uses, and anything
            // created by DCE.
        }
    }

    /// The editor's working set.
    fn cx(&mut self) -> SplitCx<'_> {
        SplitCx {
            lis: &mut *self.lis,
            vrm: &mut self.matrix.vrm,
            sa: &mut self.ss.sa,
            env: &*self.ss.env,
            mbfi: Some(&self.ss.mbfi),
        }
    }

    /// `SE->finish(&IntvMap)` with the weights of the new intervals.
    fn finish_editor(
        &mut self,
        se: &mut SplitEditor,
    ) -> Vec<usize> {
        let mut vrai = VirtRegAuxInfo::new(self.env, self.original);
        let mut map = Vec::new();
        {
            let mut cx = SplitCx {
                lis: &mut *self.lis,
                vrm: &mut self.matrix.vrm,
                sa: &mut self.ss.sa,
                env: &*self.ss.env,
                mbfi: Some(&self.ss.mbfi),
            };
            se.finish(&mut cx, &mut vrai, Some(&mut map));
        }
        for (reg, hints) in vrai.hints {
            self.hints.insert(reg, hints);
        }
        self.ss.env.reclass(self.lis, se.edit().new_regs());
        map
    }

    /// The registers the edit made go to the caller (`NewVRegs` of the
    /// `LiveRangeEdit`).
    fn collect_new_regs(
        &mut self,
        se: &SplitEditor,
        new_vregs: &mut Vec<u32>,
    ) {
        new_vregs.extend_from_slice(se.edit().new_regs());
    }

    /// A frequency as the pin prints it (`MBFI->printBlockFreq`): relative to
    /// the entry block, for `LLRM_GREEDY_DECISIONS`.
    fn shown(
        &self,
        cost: BlockFrequency,
    ) -> String {
        let entry = self.ss.mbfi.get_entry_freq().get_frequency().max(1);
        format!("{:.8}", cost.get_frequency() as f64 / entry as f64)
    }

    /// `tryRegionSplit`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1206>).
    fn try_region_split(
        &mut self,
        virt: &Rc<LiveInterval>,
        order: &AllocationOrder,
        new_vregs: &mut Vec<u32>,
    ) -> Option<RegId> {
        // D-RS3: shouldRegionSplitForVirtReg is true.
        let mut num_cands = 0usize;
        let spill_cost = self.calc_block_split_cost();
        let mut best_cost;

        // Check if we can split this live range around a compact region.
        let has_compact = self.calc_compact_region();
        if has_compact {
            // Yes, keep GlobalCand[0] as the compact region candidate.
            num_cands = 1;
            best_cost = BlockFrequency::max();
        } else {
            // No benefit from the compact region, our fallback will be
            // per-block splitting. Make sure we find a solution that is
            // cheaper than spilling.
            best_cost = spill_cost;
        }

        let best_cand = self.calculate_region_split_cost(order, &mut best_cost, &mut num_cands, false);

        // No solutions found, fall back to single block splitting.
        if !has_compact && best_cand == NO_CAND {
            return None;
        }

        self.do_region_split(virt, best_cand, has_compact, new_vregs)
    }

    /// `calculateRegionSplitCostAroundReg`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1239>).
    fn calculate_region_split_cost_around_reg(
        &mut self,
        phys: RegId,
        best_cost: &mut BlockFrequency,
        num_cands: &mut usize,
        best_cand: &mut usize,
    ) {
        // Discard bad candidates before we run out of interference cache
        // cursors. This will only affect register classes with a lot of
        // registers (>32).
        if *num_cands == self.ss.cache.get_max_cursors() {
            let mut worst_count = usize::MAX;
            let mut worst = 0;
            for cand_index in 0..*num_cands {
                if cand_index == *best_cand || self.ss.global_cand[cand_index].phys_reg.is_none() {
                    continue;
                }
                let count = self.ss.global_cand[cand_index].live_count();
                if count < worst_count {
                    worst = cand_index;
                    worst_count = count;
                }
            }
            *num_cands -= 1;
            // `GlobalCand[Worst] = GlobalCand[NumCands]`: the cursor of the
            // discarded candidate is released by the move.
            let last = std::mem::take(&mut self.ss.global_cand[*num_cands]);
            self.ss.global_cand[worst] = last;
            if *best_cand == *num_cands {
                *best_cand = worst;
            }
        }

        if self.ss.global_cand.len() <= *num_cands {
            self.ss.global_cand.resize_with(*num_cands + 1, GlobalSplitCandidate::default);
        }
        let cand = *num_cands;
        {
            let env = Env { matrix: &*self.matrix, indexes: &self.lis.indexes };
            self.ss.global_cand[cand].reset(&mut self.ss.cache, &env, Some(phys));
        }

        let trace = llrm_support::env_flag!("LLRM_GREEDY_DECISIONS");
        self.ss.placer.prepare();
        let mut cost = BlockFrequency(0);
        if !self.add_split_constraints(cand, &mut cost) {
            if trace {
                eprintln!("{phys:?}\tno positive bundles");
            }
            return;
        }
        if trace {
            eprint!("{phys:?}\tstatic = {}", self.shown(cost));
        }
        if cost >= *best_cost {
            if trace {
                match *best_cand {
                    NO_CAND => eprintln!(" worse than no bundles"),
                    best => eprintln!(" worse than {:?}", self.ss.global_cand[best].phys_reg),
                }
            }
            return;
        }
        if !self.grow_region(cand) {
            if trace {
                eprintln!(", cannot spill all interferences.");
            }
            return;
        }

        self.finish_placer(cand);

        // No live bundles, defer to splitSingleBlocks().
        if self.ss.global_cand[cand].live_count() == 0 {
            if trace {
                eprintln!(" no bundles.");
            }
            return;
        }

        cost = cost + self.calc_global_split_cost(cand);
        if trace {
            let bundles = bundles_shown(&self.ss.global_cand[cand].live_bundles);
            eprintln!(", total = {} with bundles{bundles}.", self.shown(cost));
        }
        if cost < *best_cost {
            *best_cand = *num_cands;
            *best_cost = cost;
        }
        *num_cands += 1;
    }

    /// `calculateRegionSplitCost`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1316>).
    fn calculate_region_split_cost(
        &mut self,
        order: &AllocationOrder,
        best_cost: &mut BlockFrequency,
        num_cands: &mut usize,
        ignore_csr: bool,
    ) -> usize {
        let mut best_cand = NO_CAND;
        for (phys, _) in order.iter() {
            if ignore_csr && is_unused_callee_saved_reg(self.env, &*self.matrix, phys) {
                continue;
            }
            self.calculate_region_split_cost_around_reg(phys, best_cost, num_cands, &mut best_cand);
        }
        best_cand
    }

    /// `calcSpillCost` (RegAllocGreedy.cpp#L2356): what spilling `virt` would
    /// cost, a reload or store for each instruction that reads or writes it,
    /// at its block's frequency.
    fn calc_spill_cost(
        &self,
        virt: &Rc<LiveInterval>,
    ) -> BlockFrequency {
        let mut cost = 0u64;
        let mut seen: Vec<(InstrKey, bool, bool)> = Vec::new();
        for operand in self.lis.operands(virt.reg()) {
            let reads = operand.reads_reg || !operand.is_def;
            match seen.iter_mut().find(|(key, ..)| *key == operand.instr) {
                Some(one) => {
                    one.1 |= reads;
                    one.2 |= operand.is_def;
                }
                None => seen.push((operand.instr, reads, operand.is_def)),
            }
        }
        for (key, reads, writes) in seen {
            let at = self.lis.indexes.get_instruction_index(key);
            let block = self.lis.indexes.get_mbb_from_index(&at) as u32;
            cost += (u64::from(reads) + u64::from(writes)) * self.block_freq(block).get_frequency();
        }
        BlockFrequency(cost)
    }

    /// The `RS_Spill` half of `tryAssignCSRFirstTime`
    /// (RegAllocGreedy.cpp#L2388): whether the callee-saved register is used
    /// (the spill costs `csr_cost` or more), else `virt` is spilled.
    pub fn csr_spill_is_dearer(
        &mut self,
        virt: &Rc<LiveInterval>,
        csr_cost: BlockFrequency,
    ) -> bool {
        self.analyze(virt);
        self.calc_spill_cost(virt) >= csr_cost
    }

    /// The `RS_New`/`RS_Assign` half of `tryAssignCSRFirstTime`
    /// (RegAllocGreedy.cpp#L2399): split `virt` around a region that costs
    /// less than `csr_cost` with the callee-saved registers ignored, if there
    /// is one (the ranges are in `new_vregs`). Whether to use the callee-saved
    /// register instead.
    pub fn csr_pre_split(
        &mut self,
        virt: &Rc<LiveInterval>,
        order: &AllocationOrder,
        csr_cost: BlockFrequency,
        new_vregs: &mut Vec<u32>,
    ) -> bool {
        self.analyze(virt);
        let mut num_cands = 0usize;
        let mut best_cost = csr_cost; // Don't modify CSRCost.
        let best_cand = self.calculate_region_split_cost(order, &mut best_cost, &mut num_cands, true);
        if best_cand == NO_CAND {
            // Use the CSR if we can't find a region split below CSRCost.
            return true;
        }
        // Perform the actual pre-splitting.
        self.do_region_split(virt, best_cand, false, new_vregs);
        false
    }

    /// `doRegionSplit`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1334>).
    fn do_region_split(
        &mut self,
        virt: &Rc<LiveInterval>,
        best_cand: usize,
        has_compact: bool,
        new_vregs: &mut Vec<u32>,
    ) -> Option<RegId> {
        let mut used_cands: Vec<usize> = Vec::new();
        // Prepare split editor.
        let mut delegate = NoDelegate;
        let mut se = SplitEditor::new();
        se.reset(self.lis, LiveRangeEdit::new(virt.reg(), &mut delegate), SPLIT_SPILL_MODE);

        // Assign all edge bundles to the preferred candidate, or NoCand.
        let nb = self.ss.bundles.get_num_bundles() as usize;
        self.ss.bundle_cand.clear();
        self.ss.bundle_cand.resize(nb, NO_CAND);

        // Assign bundles for the best candidate region.
        if best_cand != NO_CAND {
            let mut bc = std::mem::take(&mut self.ss.bundle_cand);
            let b = self.ss.global_cand[best_cand].get_bundles(&mut bc, best_cand);
            self.ss.bundle_cand = bc;
            if llrm_support::env_flag!("LLRM_GREEDY_DECISIONS") {
                eprintln!("Split for {:?} in {b} bundles", self.ss.global_cand[best_cand].phys_reg);
            }
            if b != 0 {
                used_cands.push(best_cand);
                let idx = se.open_intv(&mut self.cx());
                self.ss.global_cand[best_cand].intv_idx = idx;
            }
        }

        // Assign bundles for the compact region.
        if has_compact {
            assert!(self.ss.global_cand[0].phys_reg.is_none(), "Compact region has no physreg");
            let mut bc = std::mem::take(&mut self.ss.bundle_cand);
            let b = self.ss.global_cand[0].get_bundles(&mut bc, 0);
            self.ss.bundle_cand = bc;
            if b != 0 {
                used_cands.push(0);
                let idx = se.open_intv(&mut self.cx());
                self.ss.global_cand[0].intv_idx = idx;
            }
        }

        self.split_around_region(&mut se, &used_cands, new_vregs);
        None
    }

    /// `tryBlockSplit`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1469>).
    fn try_block_split(
        &mut self,
        virt: &Rc<LiveInterval>,
        new_vregs: &mut Vec<u32>,
    ) -> Option<RegId> {
        let single_instrs = self.env.is_proper_subclass(virt.reg());
        let mut delegate = NoDelegate;
        let mut se = SplitEditor::new();
        se.reset(self.lis, LiveRangeEdit::new(virt.reg(), &mut delegate), SPLIT_SPILL_MODE);
        let use_blocks = self.ss.sa.get_use_blocks().to_vec();
        for bi in &use_blocks {
            let mut cx = self.cx();
            if cx.sa.should_split_single_block(cx.lis, cx.vrm, cx.env, bi, single_instrs) {
                se.split_single_block(&mut cx, bi);
            }
        }
        // No blocks were split.
        if se.edit().empty() {
            return None;
        }

        // We did split for some blocks.
        let intv_map = self.finish_editor(&mut se);
        self.collect_new_regs(&se, new_vregs);

        // Sort out the new intervals created by splitting. The remainder
        // interval goes straight to spilling, the new local ranges get to stay
        // RS_New.
        for i in 0..se.edit().size() {
            let reg = se.edit().get(i);
            if self.extra.get_or_init_stage(reg) == LiveRangeStage::New && intv_map[i] == 0 {
                self.extra.set_stage(reg, LiveRangeStage::Spill);
            }
        }
        None
    }

    /// `calcGapWeights`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1666>):
    /// the maximum spill weight that needs to be evicted to use `phys` between
    /// two entries of the use slots.
    fn calc_gap_weights(
        &mut self,
        virt: &Rc<LiveInterval>,
        phys: RegId,
    ) -> Vec<f32> {
        assert!(self.ss.sa.get_use_blocks().len() == 1, "Not a local interval");
        let bi = self.ss.sa.get_use_blocks()[0];
        let uses = self.ss.sa.get_use_slots().to_vec();
        let num_gaps = uses.len() - 1;

        // Start and end points for the interference check.
        let start_idx = if bi.live_in { bi.first_instr.get_base_index() } else { bi.first_instr };
        let stop_idx = if bi.live_out { bi.last_instr.get_boundary_index() } else { bi.last_instr };

        let mut gap_weight = vec![0.0f32; num_gaps];

        // Add interference from each overlapping register.
        for unit in self.matrix.tri().regunits(phys) {
            if !self.matrix.query_check(virt.reg(), &virt.range, unit) {
                continue;
            }
            // We know that VirtReg is a continuous interval from FirstInstr to
            // LastInstr, so we don't need InterferenceQuery.
            //
            // Interference that overlaps an instruction is counted in both
            // gaps surrounding the instruction. The exception is interference
            // before StartIdx and after StopIdx.
            let Some(liu) = self.matrix.union(unit) else { continue };
            let mut int_i = liu.find(&start_idx);
            let mut gap = 0usize;
            'segments: while let Some(at) = int_i {
                let key = liu.entry(at).start;
                if key >= stop_idx {
                    break;
                }
                // Skip the gaps before IntI.
                while uses[gap + 1].get_boundary_index() < key {
                    gap += 1;
                    if gap == num_gaps {
                        break 'segments;
                    }
                }
                // Update the gaps covered by IntI.
                let (stop, vreg) = (liu.entry(at).stop, liu.entry(at).value);
                let weight = self.lis.get_interval(vreg).weight();
                while gap != num_gaps {
                    gap_weight[gap] = gap_weight[gap].max(weight);
                    if uses[gap + 1].get_base_index() >= stop {
                        break;
                    }
                    gap += 1;
                }
                if gap == num_gaps {
                    break;
                }
                int_i = liu.next(at);
            }
        }

        // Add fixed interference.
        let fixed = self.matrix.fixed();
        for unit in self.matrix.tri().regunits(phys) {
            let Some(lr) = fixed.fixed_range(unit) else { continue };
            let mut i = lr.find(&start_idx);
            let mut gap = 0usize;
            'fixed: while i != lr.segments.len() && lr.segments[i].start < stop_idx {
                while uses[gap + 1].get_boundary_index() < lr.segments[i].start {
                    gap += 1;
                    if gap == num_gaps {
                        break 'fixed;
                    }
                }
                while gap != num_gaps {
                    gap_weight[gap] = f32::INFINITY;
                    if uses[gap + 1].get_base_index() >= lr.segments[i].end {
                        break;
                    }
                    gap += 1;
                }
                if gap == num_gaps {
                    break;
                }
                i += 1;
            }
        }
        gap_weight
    }

    /// `tryInstructionSplit`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1584>):
    /// split a live range around individual instructions. Normally not
    /// worthwhile, since the spiller does the same; but when the range is in a
    /// constrained class, the pieces can move to a larger one.
    fn try_instruction_split(
        &mut self,
        virt: &Rc<LiveInterval>,
        new_vregs: &mut Vec<u32>,
    ) -> Option<RegId> {
        // There is no point to this if there are no larger sub-classes.
        // DEVIATE: no subranges (D-RS1), so that is the only way in.
        if !self.env.is_proper_subclass(virt.reg()) {
            return None;
        }
        let mut delegate = NoDelegate;
        let mut se = SplitEditor::new();
        se.reset(self.lis, LiveRangeEdit::new(virt.reg(), &mut delegate), ComplementSpillMode::Size);

        let uses = self.ss.sa.get_use_slots().to_vec();
        if uses.len() <= 1 {
            return None;
        }

        // Split around every non-copy instruction if this split will relax the
        // constraints on the virtual register. Otherwise, splitting just
        // inserts uncoalescable copies that do not help the allocation.
        for &slot in &uses {
            if let Some(key) = self.lis.indexes.get_instruction_from_index(&slot) {
                let insn = self.lis.instr(key);
                if insn.copy_of().is_some() || !self.env.constrains(insn, virt.reg()) {
                    continue;
                }
            }
            let mut cx = self.cx();
            se.open_intv(&mut cx);
            let seg_start = se.enter_intv_before(&mut cx, slot);
            let seg_stop = se.leave_intv_after(&mut cx, slot);
            se.use_intv(seg_start, seg_stop);
        }

        if se.edit().empty() {
            return None;
        }

        self.finish_editor(&mut se);
        self.collect_new_regs(&se, new_vregs);
        // Assign all new registers to RS_Spill. This was the last chance.
        for i in 0..se.edit().size() {
            self.extra.set_stage(se.edit().get(i), LiveRangeStage::Spill);
        }
        None
    }

    /// `tryLocalSplit`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1744>):
    /// split into smaller intervals inside the only basic block.
    fn try_local_split(
        &mut self,
        virt: &Rc<LiveInterval>,
        order: &AllocationOrder,
        new_vregs: &mut Vec<u32>,
    ) -> Option<RegId> {
        // TODO (in C++ too): the function currently only handles a single
        // UseBlock.
        if self.ss.sa.get_use_blocks().len() != 1 {
            return None;
        }
        let bi = self.ss.sa.get_use_blocks()[0];

        // Note that it is possible to have an interval that is live-in or
        // live-out while only covering a single block. We simply assume that
        // the interval is continuous from FirstInstr to LastInstr.
        let uses = self.ss.sa.get_use_slots().to_vec();
        if uses.len() <= 2 {
            return None;
        }
        let num_gaps = uses.len() - 1;

        // If VirtReg is live across any register mask operands, compute a list
        // of gaps with register masks.
        let mut reg_mask_gaps: Vec<usize> = Vec::new();
        let crosses_mask = self.matrix_crosses_reg_mask(virt);
        if crosses_mask {
            let (bstart, bend) = *self.lis.indexes.get_mbb_range(bi.mbb);
            let masks = self.matrix.fixed().reg_mask_range_in_block(bi.mbb, bstart, bend);
            // Only the instructions that destroy registers are regmasks
            // (`getRegMaskSlotsInBlock`).
            let fixed = self.matrix.fixed();
            let slots: Vec<SlotIndex> =
                masks.clone().filter(|m| fixed.reg_mask_destroys(*m)).map(|m| fixed.reg_mask_slot(m)).collect();
            // Constrain to VirtReg's live range.
            let mut ri = slots.partition_point(|s| *s < uses[0].get_reg_slot(false));
            let re = slots.len();
            let mut i = 0;
            while i != num_gaps && ri != re {
                // Look for Uses[I] <= RMS <= Uses[I + 1].
                assert!(!SlotIndex::is_earlier_instr(&slots[ri], &uses[i]));
                if SlotIndex::is_earlier_instr(&uses[i + 1], &slots[ri]) {
                    i += 1;
                    continue;
                }
                // Skip a regmask on the same instruction as the last use. It
                // doesn't overlap the live range.
                if SlotIndex::is_same_instr(&uses[i + 1], &slots[ri]) && i + 1 == num_gaps {
                    break;
                }
                reg_mask_gaps.push(i);
                // Advance ri to the next gap. A regmask on one of the uses
                // counts in both gaps.
                while ri != re && SlotIndex::is_earlier_instr(&slots[ri], &uses[i + 1]) {
                    ri += 1;
                }
                i += 1;
            }
        }

        // Since we allow local split results to be split again, there is a
        // risk of creating infinite loops. Instead we use these rules:
        //
        // 1. Allow any split for ranges with getStage() < RS_Split2. (Except
        //    for the noop split, of course).
        // 2. Require progress be made for ranges with getStage() == RS_Split2.
        //    All the new ranges must have fewer instructions than before the
        //    split.
        // 3. New ranges with the same number of instructions are marked
        //    RS_Split2, smaller ranges are marked RS_New.
        let progress_required = self.extra.get_stage(virt.reg()) >= LiveRangeStage::Split2;

        // Best split candidate.
        let mut best_before = num_gaps;
        let mut best_after = 0usize;
        let mut best_diff = 0.0f32;

        let block_freq = self.block_freq(bi.mbb as u32).get_frequency() as f32
            * (1.0 / self.ss.mbfi.get_entry_freq().get_frequency() as f32);

        for (phys, _) in order.iter() {
            // Keep track of the largest spill weight that would need to be
            // evicted in order to make use of PhysReg between UseSlots[I] and
            // UseSlots[I + 1].
            let mut gap_weight = self.calc_gap_weights(virt, phys);

            // Remove any gaps with regmask clobbers.
            if crosses_mask && self.matrix.check_interference(virt, phys) == super::matrix::InterferenceKind::RegMask {
                for &gap in &reg_mask_gaps {
                    gap_weight[gap] = f32::INFINITY;
                }
            }

            // Try to find the best sequence of gaps to close. The new spill
            // weight must be larger than any gap interference.

            // We will split before Uses[SplitBefore] and after
            // Uses[SplitAfter].
            let mut split_before = 0usize;
            let mut split_after = 1usize;

            // MaxGap should always be
            // max(GapWeight[SplitBefore..SplitAfter-1]).
            // It is the spill weight that needs to be evicted.
            let mut max_gap = gap_weight[0];

            loop {
                // Live before/after split?
                let live_before = split_before != 0 || bi.live_in;
                let live_after = split_after != num_gaps || bi.live_out;

                // Stop before the interval gets so big we wouldn't be making
                // progress.
                if !live_before && !live_after {
                    break;
                }
                // Should the interval be extended or shrunk?
                let mut shrink = true;

                // How many gaps would the new range have?
                let new_gaps = live_before as usize + split_after - split_before + live_after as usize;

                // Legally, without causing looping?
                let legal = !progress_required || new_gaps < num_gaps;

                if legal && max_gap < f32::INFINITY {
                    // Estimate the new spill weight. Each instruction reads or
                    // writes the register. Conservatively assume there are no
                    // read-modify-write instructions.
                    //
                    // Try to guess the size of the new interval.
                    let est_weight = normalize_spill_weight(
                        block_freq * (new_gaps + 1) as f32,
                        (uses[split_before].distance(&uses[split_after])
                            + (live_before as i32 + live_after as i32) * SlotIndex::INSTR_DIST as i32)
                            as u32,
                        1,
                    );
                    // Would this split be possible to allocate? Never allocate
                    // all gaps, we wouldn't be making progress.
                    if est_weight * HYSTERESIS >= max_gap {
                        shrink = false;
                        let diff = est_weight - max_gap;
                        if diff > best_diff {
                            best_diff = HYSTERESIS * diff;
                            best_before = split_before;
                            best_after = split_after;
                        }
                    }
                }

                // Try to shrink.
                if shrink {
                    split_before += 1;
                    if split_before < split_after {
                        // Recompute the max when necessary.
                        if gap_weight[split_before - 1] >= max_gap {
                            max_gap = gap_weight[split_before];
                            for i in split_before + 1..split_after {
                                max_gap = max_gap.max(gap_weight[i]);
                            }
                        }
                        continue;
                    }
                    max_gap = 0.0;
                }

                // Try to extend the interval.
                if split_after >= num_gaps {
                    break;
                }

                max_gap = max_gap.max(gap_weight[split_after]);
                split_after += 1;
            }
        }

        // Didn't find any candidates?
        if best_before == num_gaps {
            return None;
        }

        let mut delegate = NoDelegate;
        let mut se = SplitEditor::new();
        se.reset(self.lis, LiveRangeEdit::new(virt.reg(), &mut delegate), ComplementSpillMode::Partition);

        {
            let mut cx = self.cx();
            se.open_intv(&mut cx);
            let seg_start = se.enter_intv_before(&mut cx, uses[best_before]);
            let seg_stop = se.leave_intv_after(&mut cx, uses[best_after]);
            se.use_intv(seg_start, seg_stop);
        }
        let intv_map = self.finish_editor(&mut se);
        self.collect_new_regs(&se, new_vregs);

        // If the new range has the same number of instructions as before, mark
        // it as RS_Split2 so the next split will be forced to make progress.
        // Otherwise, leave the new intervals as RS_New so they can compete.
        let live_before = best_before != 0 || bi.live_in;
        let live_after = best_after != num_gaps || bi.live_out;
        let new_gaps = live_before as usize + best_after - best_before + live_after as usize;
        if new_gaps >= num_gaps {
            assert!(!progress_required, "Didn't make progress when it was required.");
            for (i, &m) in intv_map.iter().enumerate() {
                if m == 1 {
                    self.extra.set_stage(se.edit().get(i), LiveRangeStage::Split2);
                }
            }
        }
        None
    }

    /// `Matrix->checkRegMaskInterference(VirtReg)` with no register: whether
    /// the range is live across any regmask.
    fn matrix_crosses_reg_mask(
        &mut self,
        virt: &Rc<LiveInterval>,
    ) -> bool {
        self.matrix.crosses_reg_mask(virt)
    }
}

/// The live bundles of a candidate as the pin prints them: ` EB#1 EB#14`.
fn bundles_shown(live: &[bool]) -> String {
    live.iter().enumerate().filter(|(_, live)| **live).map(|(at, _)| format!(" EB#{at}")).collect()
}

#[cfg(test)]
mod tests {
    /// The candidate line of `LLRM_GREEDY_DECISIONS` names the live bundles as
    /// the pin does (`with bundles EB#1 EB#14`), so two builds' lines diff.
    #[test]
    fn test_a_candidates_live_bundles_are_shown_as_the_pin_prints_them() {
        assert_eq!(super::bundles_shown(&[false, true, false, true]), " EB#1 EB#3");
        assert_eq!(super::bundles_shown(&[false, false]), "");
    }
}
