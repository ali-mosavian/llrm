//! `InterferenceCache`: for a physical register, the first and last point of
//! interference in each block. Translated from
//! llvm/lib/CodeGen/InterferenceCache.{h,cpp} at d1106deb71cc.
//!
//! # How it works
//! Thirty-two entries are kept in round-robin order, one per physical register
//! in use. An entry holds, for each block, the first and last slot where the
//! register's units are taken by an assigned virtual register, a fixed range
//! or a regmask. A block is computed when asked (`update`) and the blocks after
//! it that have no interference come with it. An entry is tagged with the
//! contents of the unions of its units: when one changed, every block is stale
//! (`revalidate`). A [`Cursor`] holds an entry (so it is not reused) and
//! reads a block of it.
//!
//! DEVIATE D-IC1: C++ holds `LiveIntervalUnion *`, `SlotIndexes *` and
//! `LiveIntervals *` in the cache and the cursor; here the caller passes them
//! as an [`Env`] on each call. The fact: Rust cannot hold borrows of the
//! allocator's matrix across its mutations. `IntervalMap::const_iterator` is
//! the start key of an entry of the union's `BTreeMap`; `advanceTo` steps a
//! few entries and then searches, as `IntervalMap` does.
//!
//! DEVIATE D-IC2: see [`PhysInterference::reg_mask_clobbers`]: a regmask
//! clobbers a register by the roots a call destroys.
//!
//! # Example
//! `%1` assigned to `ax` holds `[16,40)` of a block `[8,56)`; the cursor of
//! `ax` at that block has `first() = 16` and `last() = 40`, and `bx` has none.
//!
//! # Used by
//! `SplitAnalysis` and `RAGreedy::tryRegionSplit`.

use std::cell::Cell;
use std::rc::Rc;

use llrm_lir::registers::RegId;

use super::matrix::LiveRegMatrix;
use crate::liveness::liveintervalunion::{LiveIntervalUnion, Pos};
use crate::liveness::slotindex::{SlotIndex, SlotIndexes};

/// `CacheEntries`.
const CACHE_ENTRIES: usize = 32;

/// What the C++ cache keeps pointers to (D-IC1).
pub struct Env<'a> {
    /// `LIUArray`, `TRI` and `LIS` (the fixed constraints).
    pub matrix: &'a LiveRegMatrix,
    pub indexes: &'a SlotIndexes,
}

/// `InterferenceCache::BlockInterference`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/InterferenceCache.h#L35>).
#[derive(Clone, Copy, Default)]
struct BlockInterference {
    tag: u32,
    first: SlotIndex,
    last: SlotIndex,
}

/// `Entry::RegUnitInfo`.
struct RegUnitInfo {
    unit: u32,
    /// The union's tag when this entry last looked.
    virt_tag: u32,
    /// `VirtI`: an entry of the union; None at the end.
    virt_i: Option<Pos>,
    /// `FixedI`: index into the fixed range's segments.
    fixed_i: usize,
}

/// `InterferenceCache::Entry`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/InterferenceCache.h#L45>).
#[derive(Default)]
struct Entry {
    phys_reg: Option<RegId>,
    tag: u32,
    ref_count: Rc<Cell<u32>>,
    prev_pos: SlotIndex,
    reg_units: Vec<RegUnitInfo>,
    blocks: Vec<BlockInterference>,
    /// `PhysInterference::lane_of` when the blocks were read: a regmask that
    /// clobbers a value only where a wide read follows is another answer for
    /// another value.
    lane: Option<u32>,
}

impl Entry {
    fn has_refs(&self) -> bool {
        self.ref_count.get() > 0
    }

    /// `valid`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/InterferenceCache.cpp#L118>).
    fn valid(
        &self,
        env: &Env,
    ) -> bool {
        let phys = self.phys_reg.expect("an entry in use");
        let mut i = 0;
        for unit in env.matrix.tri().regunits(phys) {
            if i == self.reg_units.len() {
                return false;
            }
            if env.matrix.union(unit).is_some_and(|liu| liu.changed_since(self.reg_units[i].virt_tag)) {
                return false;
            }
            i += 1;
        }
        i == self.reg_units.len() && self.lane == env.matrix.fixed().lane_of(phys)
    }

    /// `revalidate`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/InterferenceCache.cpp#L88>):
    /// the unions' contents have changed, update tags.
    fn revalidate(
        &mut self,
        env: &Env,
    ) {
        // Invalidate all block entries.
        self.tag += 1;
        // Invalidate all iterators.
        self.prev_pos = SlotIndex::default();
        let phys = self.phys_reg.expect("an entry in use");
        self.lane = env.matrix.fixed().lane_of(phys);
        for (i, unit) in env.matrix.tri().regunits(phys).enumerate() {
            self.reg_units[i].virt_tag = env.matrix.union(unit).map_or(0, |liu| liu.get_tag());
        }
    }

    /// `reset`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/InterferenceCache.cpp#L99>).
    fn reset(
        &mut self,
        phys: RegId,
        env: &Env,
        num_blocks: usize,
    ) {
        assert!(!self.has_refs(), "Cannot reset cache entry with references");
        // The unions' contents changed, invalidate cache.
        self.tag += 1;
        self.phys_reg = Some(phys);
        self.lane = env.matrix.fixed().lane_of(phys);
        self.blocks.resize(num_blocks, BlockInterference::default());
        // Reset iterators.
        self.prev_pos = SlotIndex::default();
        self.reg_units.clear();
        for unit in env.matrix.tri().regunits(phys) {
            let virt_tag = env.matrix.union(unit).map_or(0, |liu| liu.get_tag());
            self.reg_units.push(RegUnitInfo { unit, virt_tag, virt_i: None, fixed_i: 0 });
        }
    }

    /// `get`: the interference of block `num`, computed if stale.
    fn get(
        &mut self,
        env: &Env,
        num: usize,
    ) -> (SlotIndex, SlotIndex) {
        if self.blocks[num].tag != self.tag {
            self.update(env, num);
        }
        (self.blocks[num].first, self.blocks[num].last)
    }

    /// `update`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/InterferenceCache.cpp#L131>).
    fn update(
        &mut self,
        env: &Env,
        mut mbb_num: usize,
    ) {
        let phys = self.phys_reg.expect("an entry in use");
        let fixed = env.matrix.fixed();
        let empty = LiveIntervalUnion::new();
        let liu = |unit: u32| env.matrix.union(unit).unwrap_or(&empty);
        let (mut start, mut stop) = *env.indexes.get_mbb_range(mbb_num);

        // Use advanceTo only when possible.
        if self.prev_pos != start {
            if !self.prev_pos.is_valid() || start < self.prev_pos {
                for rui in &mut self.reg_units {
                    rui.virt_i = liu(rui.unit).find(&start);
                    rui.fixed_i = fixed.fixed_range(rui.unit).map_or(0, |f| f.find(&start));
                }
            } else {
                for rui in &mut self.reg_units {
                    rui.virt_i = liu(rui.unit).advance_to(rui.virt_i, &start);
                    if let Some(f) = fixed.fixed_range(rui.unit) {
                        if rui.fixed_i != f.segments.len() {
                            rui.fixed_i = f.advance_to(rui.fixed_i, &start);
                        }
                    }
                }
            }
            self.prev_pos = start;
        }

        let mut first;
        let mut last = SlotIndex::default();
        let mut masks;
        loop {
            self.blocks[mbb_num].tag = self.tag;
            first = SlotIndex::default();

            // Check for first interference from virtregs.
            for rui in &self.reg_units {
                let Some(i) = rui.virt_i.map(|at| liu(rui.unit).entry(at).start) else { continue };
                if i >= stop {
                    continue;
                }
                if !first.is_valid() || i < first {
                    first = i;
                }
            }

            // Same thing for fixed interference.
            for rui in &self.reg_units {
                let Some(f) = fixed.fixed_range(rui.unit) else { continue };
                if rui.fixed_i == f.segments.len() {
                    continue;
                }
                let start_i = f.segments[rui.fixed_i].start;
                if start_i >= stop {
                    continue;
                }
                if !first.is_valid() || start_i < first {
                    first = start_i;
                }
            }

            // Also check for register mask interference.
            masks = fixed.reg_mask_range_in_block(mbb_num, start, stop);
            let limit = if first.is_valid() { first } else { stop };
            for i in masks.clone() {
                if fixed.reg_mask_slot(i) >= limit {
                    break;
                }
                if fixed.reg_mask_clobbers(i, phys) {
                    // Register mask i clobbers PhysReg before the LIU
                    // interference.
                    first = fixed.reg_mask_slot(i);
                    break;
                }
            }

            self.prev_pos = stop;
            self.blocks[mbb_num].first = first;
            self.blocks[mbb_num].last = SlotIndex::default();
            if first.is_valid() {
                break;
            }

            // No interference in this block? Go ahead and precompute the next
            // block.
            mbb_num += 1;
            if mbb_num == self.blocks.len() {
                return;
            }
            if self.blocks[mbb_num].tag == self.tag {
                return;
            }
            (start, stop) = *env.indexes.get_mbb_range(mbb_num);
        }

        // Check for last interference in block.
        for rui in &mut self.reg_units {
            let u = liu(rui.unit);
            let Some(at) = rui.virt_i else { continue };
            if u.entry(at).start >= stop {
                continue;
            }
            rui.virt_i = u.advance_to(Some(at), &stop);
            let backup = rui.virt_i.is_none_or(|at| u.entry(at).start >= stop);
            let at = if backup { u.prev(rui.virt_i) } else { rui.virt_i };
            let stop_i = u.entry(at.expect("an entry before the end")).stop;
            if !last.is_valid() || stop_i > last {
                last = stop_i;
            }
        }

        // Fixed interference.
        for rui in &mut self.reg_units {
            let Some(f) = fixed.fixed_range(rui.unit) else { continue };
            if rui.fixed_i == f.segments.len() || f.segments[rui.fixed_i].start >= stop {
                continue;
            }
            rui.fixed_i = f.advance_to(rui.fixed_i, &stop);
            let backup = rui.fixed_i == f.segments.len() || f.segments[rui.fixed_i].start >= stop;
            let at = if backup { rui.fixed_i - 1 } else { rui.fixed_i };
            let stop_i = f.segments[at].end;
            if !last.is_valid() || stop_i > last {
                last = stop_i;
            }
        }

        // Also check for register mask interference.
        let limit = if last.is_valid() { last } else { start };
        for i in masks.rev() {
            if fixed.reg_mask_slot(i).get_dead_slot() <= limit {
                break;
            }
            if fixed.reg_mask_clobbers(i, phys) {
                // Register mask i clobbers PhysReg after the LIU interference.
                // Model the regmask clobber as a dead def.
                last = fixed.reg_mask_slot(i).get_dead_slot();
                break;
            }
        }
        self.blocks[mbb_num].last = last;
    }
}

/// `InterferenceCache`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/InterferenceCache.h#L32>).
pub struct InterferenceCache {
    num_blocks: usize,
    /// `PhysRegEntries`: the entry last given to each register; valid only
    /// while that entry still names the register.
    phys_reg_entries: Vec<u8>,
    /// Next round-robin entry to be picked.
    round_robin: usize,
    entries: Vec<Entry>,
}

impl InterferenceCache {
    /// `init`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/InterferenceCache.cpp#L49>):
    /// `num_blocks` is `MF->getNumBlockIDs()`.
    pub fn new(num_blocks: usize) -> Self {
        Self {
            num_blocks,
            phys_reg_entries: Vec::new(),
            round_robin: 0,
            entries: (0..CACHE_ENTRIES).map(|_| Entry::default()).collect(),
        }
    }

    /// `getMaxCursors`.
    pub fn get_max_cursors(&self) -> usize {
        CACHE_ENTRIES
    }

    /// `get`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/InterferenceCache.cpp#L62>):
    /// a valid entry for `phys`.
    fn get(
        &mut self,
        env: &Env,
        phys: RegId,
    ) -> usize {
        let at = phys.index();
        if self.phys_reg_entries.len() <= at {
            self.phys_reg_entries.resize(at + 1, u8::MAX);
        }
        let e = self.phys_reg_entries[at] as usize;
        if e < CACHE_ENTRIES && self.entries[e].phys_reg == Some(phys) {
            if !self.entries[e].valid(env) {
                self.entries[e].revalidate(env);
            }
            return e;
        }
        // No valid entry exists, pick the next round-robin entry.
        let mut e = self.round_robin;
        self.round_robin += 1;
        if self.round_robin == CACHE_ENTRIES {
            self.round_robin = 0;
        }
        for _ in 0..CACHE_ENTRIES {
            // Skip entries that are in use.
            if self.entries[e].has_refs() {
                e += 1;
                if e == CACHE_ENTRIES {
                    e = 0;
                }
                continue;
            }
            self.entries[e].reset(phys, env, self.num_blocks);
            self.phys_reg_entries[at] = e as u8;
            return e;
        }
        unreachable!("Ran out of interference cache entries.");
    }
}

/// `InterferenceCache::Cursor`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/InterferenceCache.h#L177>).
/// Dropping a cursor releases its entry.
#[derive(Default)]
pub struct Cursor {
    entry: Option<(usize, Rc<Cell<u32>>)>,
    /// `Current`: (first, last) of the block; invalid first is none
    /// (`NoInterference`).
    current: (SlotIndex, SlotIndex),
}

impl Cursor {
    pub fn new() -> Self {
        Self::default()
    }

    fn set_entry(
        &mut self,
        e: Option<(usize, Rc<Cell<u32>>)>,
    ) {
        self.current = (SlotIndex::default(), SlotIndex::default());
        if let Some((_, refs)) = &self.entry {
            refs.set(refs.get() - 1);
        }
        if let Some((_, refs)) = &e {
            refs.set(refs.get() + 1);
        }
        self.entry = e;
    }

    /// `setPhysReg`.
    pub fn set_phys_reg(
        &mut self,
        cache: &mut InterferenceCache,
        env: &Env,
        phys: Option<RegId>,
    ) {
        // Release reference before getting a new one. That guarantees we can
        // actually have CacheEntries live cursors.
        self.set_entry(None);
        if let Some(phys) = phys {
            let e = cache.get(env, phys);
            self.set_entry(Some((e, Rc::clone(&cache.entries[e].ref_count))));
        }
    }

    /// `moveToBlock`.
    pub fn move_to_block(
        &mut self,
        cache: &mut InterferenceCache,
        env: &Env,
        mbb_num: usize,
    ) {
        self.current = match &self.entry {
            Some((e, _)) => cache.entries[*e].get(env, mbb_num),
            None => (SlotIndex::default(), SlotIndex::default()),
        };
    }

    /// `hasInterference`.
    pub fn has_interference(&self) -> bool {
        self.current.0.is_valid()
    }

    /// `first`.
    pub fn first(&self) -> SlotIndex {
        self.current.0
    }

    /// `last`.
    pub fn last(&self) -> SlotIndex {
        self.current.1
    }
}

impl Drop for Cursor {
    fn drop(&mut self) {
        self.set_entry(None);
    }
}

impl Clone for Cursor {
    /// `Cursor(const Cursor &O)`: shares the entry, not the block.
    fn clone(&self) -> Self {
        let mut c = Cursor::new();
        c.set_entry(self.entry.clone());
        c
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_lir::registers::Regs;

    use super::*;
    use crate::greedy::matrix::{NoFixed, PhysInterference};
    use crate::greedy::virtregmap::RegisterInfo;
    use crate::liveness::liverange::{LiveInterval, Segment};
    use crate::model::lir::{Block, Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    /// Three blocks of ten instructions; `at[b][i]` is the reg slot of
    /// instruction `i` of block `b`.
    fn body() -> (SlotIndexes, Vec<Vec<SlotIndex>>) {
        let insns: Vec<Vec<Arc<Insn>>> =
            (0..3).map(|_| (0..10).map(|_| Arc::new(Insn::new(0, None, None, vec![], vec![]))).collect()).collect();
        let blocks = insns.iter().enumerate().map(|(b, i)| Block::from(LirBlock::new(b as i64, i.clone()))).collect();
        let body = LirBody::new("cache", 0, blocks, IndexMap::default(), IndexMap::default());
        let indexes = SlotIndexes::new(&body);
        let at = insns
            .iter()
            .map(|b| b.iter().map(|i| indexes.get_instruction_index(i.id()).get_reg_slot(false)).collect())
            .collect();
        (indexes, at)
    }

    fn interval(
        reg: u32,
        from: SlotIndex,
        to: SlotIndex,
    ) -> LiveInterval {
        let mut li = LiveInterval::new(reg, 1.0);
        let v = li.get_next_value(from);
        li.add_segment(Segment::new(from, to, Some(v)));
        li
    }

    fn matrix(fixed: Box<dyn PhysInterference>) -> LiveRegMatrix {
        LiveRegMatrix::new(RegisterInfo::new(Regs(&llrm_x86_m16::REGISTER_INFO)), fixed)
    }

    fn read(
        cache: &mut InterferenceCache,
        env: &Env,
        phys: RegId,
        block: usize,
    ) -> (bool, SlotIndex, SlotIndex) {
        let mut c = Cursor::new();
        c.set_phys_reg(cache, env, Some(phys));
        c.move_to_block(cache, env, block);
        (c.has_interference(), c.first(), c.last())
    }

    #[test]
    fn first_and_last_interference_are_the_assigned_segment_inside_the_block() {
        let (indexes, at) = body();
        let mut m = matrix(Box::new(NoFixed));
        m.assign(&interval(1, at[1][2], at[1][6]), RegId::AX);
        let env = Env { matrix: &m, indexes: &indexes };
        let mut cache = InterferenceCache::new(3);
        assert_eq!(read(&mut cache, &env, RegId::AX, 1), (true, at[1][2], at[1][6]));
        assert!(!read(&mut cache, &env, RegId::AX, 0).0);
        assert!(!read(&mut cache, &env, RegId::AX, 2).0);
        assert!(!read(&mut cache, &env, RegId::BX, 1).0, "another register has none");
        // A register that shares a unit sees it.
        assert!(read(&mut cache, &env, RegId::AL, 1).0);
    }

    #[test]
    fn a_segment_across_blocks_keeps_its_own_ends() {
        let (indexes, at) = body();
        let mut m = matrix(Box::new(NoFixed));
        m.assign(&interval(1, at[0][5], at[2][3]), RegId::AX);
        let env = Env { matrix: &m, indexes: &indexes };
        let mut cache = InterferenceCache::new(3);
        let (has, first, last) = read(&mut cache, &env, RegId::AX, 1);
        assert!(has);
        // The segment's own ends, not clipped to the block (`I.start()`,
        // `I.stop()`): the users compare them with the block.
        assert_eq!(first, at[0][5]);
        assert_eq!(last, at[2][3]);
        assert_eq!(read(&mut cache, &env, RegId::AX, 2), (true, at[0][5], at[2][3]));
    }

    /// `advanceTo` and `--I` searched the union's `BTreeMap` for each step it
    /// made: walking 3 blocks of 9 entries each took 20 searches (d_faces -O2
    /// m32, `Cursor::move_to_block` 20.0 Minstr against LLVM's 4.0 for
    /// `Entry::update`). An iterator held with its stop steps for free.
    #[test]
    fn walking_the_blocks_of_a_union_searches_its_map_a_few_times_per_block() {
        let (indexes, at) = body();
        let mut m = matrix(Box::new(NoFixed));
        for b in 0..3 {
            for i in 0..9 {
                m.assign(&interval(1 + (b * 9 + i) as u32, at[b][i], at[b][i + 1]), RegId::AX);
            }
        }
        let env = Env { matrix: &m, indexes: &indexes };
        let mut cache = InterferenceCache::new(3);
        let mut c = Cursor::new();
        c.set_phys_reg(&mut cache, &env, Some(RegId::AX));
        let before = crate::liveness::intervalmap::searches();
        for b in 0..3 {
            c.move_to_block(&mut cache, &env, b);
            assert_eq!((c.first(), c.last()), (at[b][0], at[b][9]));
        }
        let searched = crate::liveness::intervalmap::searches() - before;
        assert!(
            searched <= 10,
            "3 blocks of 9 entries searched the map {searched} times (10 allowed; 20 when the map is a BTreeMap)"
        );
    }

    /// A stale answer after `unassign` would make a split region think the
    /// register is taken where it is free.
    #[test]
    fn a_cached_block_is_recomputed_after_the_union_changes() {
        let (indexes, at) = body();
        let mut m = matrix(Box::new(NoFixed));
        let li = interval(1, at[1][2], at[1][6]);
        m.assign(&li, RegId::AX);
        let mut cache = InterferenceCache::new(3);
        assert!(read(&mut cache, &Env { matrix: &m, indexes: &indexes }, RegId::AX, 1).0);
        m.unassign(&li);
        assert!(!read(&mut cache, &Env { matrix: &m, indexes: &indexes }, RegId::AX, 1).0);
        let other = interval(2, at[1][7], at[1][8]);
        m.assign(&other, RegId::AX);
        assert_eq!(read(&mut cache, &Env { matrix: &m, indexes: &indexes }, RegId::AX, 1), (true, at[1][7], at[1][8]));
    }

    struct Call(SlotIndex, RegId);

    impl PhysInterference for Call {
        fn reg_mask_clobbered(
            &self,
            _li: &LiveInterval,
        ) -> Option<llrm_lir::registers::RegSet> {
            None
        }

        fn reg_unit_interferes(
            &self,
            _li: &LiveInterval,
            _phys: RegId,
        ) -> bool {
            false
        }

        fn reg_mask_range(
            &self,
            start: SlotIndex,
            stop: SlotIndex,
        ) -> std::ops::Range<usize> {
            if start <= self.0 && self.0 < stop { 0..1 } else { 0..0 }
        }

        fn reg_mask_slot(
            &self,
            _i: usize,
        ) -> SlotIndex {
            self.0
        }

        fn reg_mask_clobbers(
            &self,
            _i: usize,
            phys: RegId,
        ) -> bool {
            crate::model::ir::root(phys) == crate::model::ir::root(self.1)
        }
    }

    #[test]
    fn a_call_that_destroys_the_register_is_interference_as_a_dead_def() {
        let (indexes, at) = body();
        let m = matrix(Box::new(Call(at[1][4], RegId::AX)));
        let env = Env { matrix: &m, indexes: &indexes };
        let mut cache = InterferenceCache::new(3);
        assert_eq!(read(&mut cache, &env, RegId::AX, 1), (true, at[1][4], at[1][4].get_dead_slot()));
        assert!(!read(&mut cache, &env, RegId::AX, 0).0);
        assert!(!read(&mut cache, &env, RegId::BX, 1).0);
    }

    #[test]
    fn a_cursor_keeps_its_entry_from_being_reused() {
        let (indexes, at) = body();
        let mut m = matrix(Box::new(NoFixed));
        m.assign(&interval(1, at[1][2], at[1][6]), RegId::AX);
        let env = Env { matrix: &m, indexes: &indexes };
        let mut cache = InterferenceCache::new(3);
        let mut held = Cursor::new();
        held.set_phys_reg(&mut cache, &env, Some(RegId::AX));
        let e = held.entry.as_ref().unwrap().0;
        for _ in 0..CACHE_ENTRIES + 3 {
            let mut c = Cursor::new();
            c.set_phys_reg(&mut cache, &env, Some(RegId::BX));
            c.set_phys_reg(&mut cache, &env, Some(RegId::CX));
            c.set_phys_reg(&mut cache, &env, Some(RegId::DX));
        }
        assert_eq!(cache.entries[e].phys_reg, Some(RegId::AX));
        held.move_to_block(&mut cache, &env, 1);
        assert_eq!(held.first(), at[1][2]);
        drop(held);
        assert!(!cache.entries[e].has_refs());
    }
}
