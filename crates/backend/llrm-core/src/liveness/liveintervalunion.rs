//! `LiveIntervalUnion`: the segments of the virtual registers assigned to one
//! register unit, with the `Query` that finds which of them interfere with a
//! live range. Translated from llvm/include/llvm/CodeGen/LiveIntervalUnion.h
//! and llvm/lib/CodeGen/LiveIntervalUnion.cpp at d1106deb71cc.
//!
//! # How it works
//! The union is an ordered map from the start of a segment to its end and the
//! virtual register that holds it. Adjacent segments of one register are one
//! entry (as `IntervalMap::insert` makes them). A `Query` walks a live range
//! and the union together, advancing whichever ends first, and records each
//! register the range overlaps; it keeps its place, so asking for the first
//! interference and then for all of them costs one walk, and a query on an
//! unchanged union and range is answered from what it found.
//!
//! ```text
//!   union   [16,40):%3   [48,64):%7        (one register unit)
//!   range   [32,56)                        overlaps %3 and %7
//! ```
//!
//! DEVIATE (container, number to follow from the shadow core): `IntervalMap`
//! (llvm/include/llvm/ADT/IntervalMap.h, a B+ tree with inline small nodes) is
//! a `BTreeMap` keyed by the start of an entry; its iterators (`find`,
//! `advanceTo`, `++`) are `range` lookups on the key. A virtual register is its
//! number.
//!
//! # Example
//! A register with segments `[16,40)` and `[40,48)` is one entry `[16,48)`;
//! extracting it removes that entry whole.
//!
//! # Used by
//! `liveregmatrix`.

use std::collections::BTreeMap;
use std::ops::Bound;

use super::liverange::LiveRange;
use super::slotindex::SlotIndex;

/// A virtual register, as `const LiveInterval *` is in C++.
pub type VReg = u32;

/// `LiveIntervalUnion` (LiveIntervalUnion.h#L42).
#[derive(Default)]
pub struct LiveIntervalUnion {
    /// start -> (stop, register): `IntervalMap<SlotIndex, const LiveInterval
    /// *>`.
    segments: BTreeMap<SlotIndex, (SlotIndex, VReg)>,
    /// A unique tag for the current contents.
    tag: u32,
}

impl LiveIntervalUnion {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn start_index(&self) -> SlotIndex {
        self.segments.keys().next().expect("a segment").clone()
    }

    pub fn end_index(&self) -> SlotIndex {
        self.segments.values().next_back().expect("a segment").0.clone()
    }

    /// `getTag`: an opaque tag representing the current state of the union.
    pub fn get_tag(&self) -> u32 {
        self.tag
    }

    /// `changedSince`.
    pub fn changed_since(
        &self,
        tag: u32,
    ) -> bool {
        tag != self.tag
    }

    /// The entries in order: (start, stop, register).
    pub fn iter(&self) -> impl Iterator<Item = (&SlotIndex, &SlotIndex, VReg)> {
        self.segments.iter().map(|(start, (stop, reg))| (start, stop, *reg))
    }

    /// `IntervalMap::find(x)`: the entry with the first stop after `x`: the one
    /// containing it, or the first after the hole it is in. None at the end.
    pub fn find(
        &self,
        x: &SlotIndex,
    ) -> Option<SlotIndex> {
        if let Some((start, (stop, _))) = self.segments.range(..=x.clone()).next_back() {
            if stop > x {
                return Some(start.clone());
            }
        }
        self.segments.range((Bound::Excluded(x.clone()), Bound::Unbounded)).next().map(|(start, _)| start.clone())
    }

    /// The entry after the one starting at `key`.
    pub fn next(
        &self,
        key: &SlotIndex,
    ) -> Option<SlotIndex> {
        self.segments.range((Bound::Excluded(key.clone()), Bound::Unbounded)).next().map(|(start, _)| start.clone())
    }

    /// `--I`: the entry before the one starting at `key`; the last entry from
    /// the end (`None`).
    pub fn prev(
        &self,
        key: Option<&SlotIndex>,
    ) -> Option<SlotIndex> {
        match key {
            Some(key) => self.segments.range(..*key).next_back().map(|(start, _)| *start),
            None => self.segments.keys().next_back().copied(),
        }
    }

    /// `I.stop()` of the entry starting at `key`.
    pub fn stop_of(
        &self,
        key: &SlotIndex,
    ) -> SlotIndex {
        self.segments[key].0
    }

    /// `IntervalMap::insert(start, stop, reg)`: adjacent entries of the same
    /// register become one.
    fn insert(
        &mut self,
        mut start: SlotIndex,
        mut stop: SlotIndex,
        reg: VReg,
    ) {
        if let Some((left, (left_stop, left_reg))) = self.segments.range(..start.clone()).next_back() {
            if *left_stop == start && *left_reg == reg {
                let left = left.clone();
                start = left.clone();
                self.segments.remove(&left);
            }
        }
        if let Some((right_stop, right_reg)) = self.segments.get(&stop).cloned() {
            if right_reg == reg {
                self.segments.remove(&stop);
                stop = right_stop;
            }
        }
        self.segments.insert(start, (stop, reg));
    }

    /// `unify` (LiveIntervalUnion.cpp#L28): adds a live virtual register and
    /// merges its segments.
    pub fn unify(
        &mut self,
        virt_reg: VReg,
        range: &LiveRange,
    ) {
        if range.empty() {
            return;
        }
        self.tag += 1;
        // Insert each of the virtual register's live segments into the map.
        for s in &range.segments {
            self.insert(s.start.clone(), s.end.clone(), virt_reg);
        }
    }

    /// `extract` (LiveIntervalUnion.cpp#L53): removes a live virtual
    /// register's segments from this union.
    pub fn extract(
        &mut self,
        virt_reg: VReg,
        range: &LiveRange,
    ) {
        if range.empty() {
            return;
        }
        self.tag += 1;
        // Remove each of the virtual register's live segments from the map.
        let mut reg_pos = 0usize;
        let mut seg_pos = self.find(&range.segments[0].start);
        while let Some(key) = seg_pos {
            let (_, reg) = self.segments.remove(&key).expect("an entry");
            assert_eq!(reg, virt_reg, "Inconsistent LiveInterval");
            // The iterator is now at the entry after the one erased.
            let Some(next) =
                self.segments.range((Bound::Included(key.clone()), Bound::Unbounded)).next().map(|(s, _)| s.clone())
            else {
                return;
            };
            // Skip all segments that may have been coalesced.
            reg_pos = range.advance_to(reg_pos, &next);
            if reg_pos == range.segments.len() {
                return;
            }
            seg_pos = self.find(&range.segments[reg_pos].start);
        }
    }

    /// `clearAllSegmentsReferencing` (LiveIntervalUnion.cpp#L84): removes all
    /// segments referencing `virt_reg`.
    pub fn clear_all_segments_referencing(
        &mut self,
        virt_reg: VReg,
    ) {
        self.tag += 1;
        self.segments.retain(|_, (_, reg)| *reg != virt_reg);
    }

    /// `clear`: removes all inserted virtual registers.
    pub fn clear(&mut self) {
        self.segments.clear();
        self.tag += 1;
    }

    /// `getOneVReg` (LiveIntervalUnion.cpp#L111): any virtual register assigned
    /// to this unit.
    pub fn get_one_vreg(&self) -> Option<VReg> {
        self.segments.values().next().map(|(_, reg)| *reg)
    }

    /// `print` (LiveIntervalUnion.cpp#L97).
    pub fn print(&self) -> String {
        if self.empty() {
            return " empty\n".to_owned();
        }
        let mut out = String::new();
        for (start, stop, reg) in self.iter() {
            out.push_str(&format!(" [{start} {stop}):%{reg}"));
        }
        out.push('\n');
        out
    }
}

/// `LiveIntervalUnion::Query` (LiveIntervalUnion.h#L99): interferences between
/// a single live virtual register and a union. The query holds its place; the
/// range and union it is about are passed to each call (they are owned
/// elsewhere), and `init` forgets what it found when the user tag, the range
/// (`lr_key`), the union (`union_key`) or the union's contents changed.
#[derive(Default)]
pub struct Query {
    lr_key: Option<u64>,
    union_key: Option<u64>,
    /// Current position in LR.
    lri: usize,
    /// Current position in the union: the start of an entry; None at the end.
    live_union_i: Option<SlotIndex>,
    interfering_vregs: Vec<VReg>,
    checked_first_interference: bool,
    seen_all_interferences: bool,
    tag: u32,
    user_tag: u32,
}

impl Query {
    pub fn new() -> Self {
        Self::default()
    }

    /// `reset`.
    pub fn reset(
        &mut self,
        new_user_tag: u32,
        lr_key: u64,
        union_key: u64,
        union_tag: u32,
    ) {
        self.lr_key = Some(lr_key);
        self.union_key = Some(union_key);
        self.interfering_vregs.clear();
        self.checked_first_interference = false;
        self.seen_all_interferences = false;
        self.tag = union_tag;
        self.user_tag = new_user_tag;
    }

    /// `init`: retains the results when nothing it depends on changed.
    pub fn init(
        &mut self,
        new_user_tag: u32,
        lr_key: u64,
        union_key: u64,
        union: &LiveIntervalUnion,
    ) {
        if self.user_tag == new_user_tag
            && self.lr_key == Some(lr_key)
            && self.union_key == Some(union_key)
            && !union.changed_since(self.tag)
        {
            // Retain cached results, e.g. firstInterference.
            return;
        }
        self.reset(new_user_tag, lr_key, union_key, union.get_tag());
    }

    /// `isSeenInterference`.
    fn is_seen_interference(
        &self,
        virt_reg: VReg,
    ) -> bool {
        self.interfering_vregs.contains(&virt_reg)
    }

    /// `checkInterference`: does this live virtual register interfere with the
    /// union?
    pub fn check_interference(
        &mut self,
        lr: &LiveRange,
        union: &LiveIntervalUnion,
    ) -> bool {
        self.collect_interfering_vregs(lr, union, 1) != 0
    }

    /// `interferingVRegs`: the vector `collectInterferingVRegs` generated.
    pub fn interfering_vregs(
        &mut self,
        lr: &LiveRange,
        union: &LiveIntervalUnion,
        max_interfering_regs: usize,
    ) -> &[VReg] {
        if !self.seen_all_interferences || max_interfering_regs < self.interfering_vregs.len() {
            self.collect_interfering_vregs(lr, union, max_interfering_regs);
        }
        &self.interfering_vregs
    }

    /// `collectInterferingVRegs` (LiveIntervalUnion.cpp#L141): collects the
    /// virtual registers in the union that interfere with the range, up to
    /// `max_interfering_regs`.
    ///
    /// The query state is one of: not yet checked (the iterators are unset),
    /// all interferences seen, or the iterators left at the last seen
    /// intersection.
    pub fn collect_interfering_vregs(
        &mut self,
        lr: &LiveRange,
        union: &LiveIntervalUnion,
        max_interfering_regs: usize,
    ) -> usize {
        // Fast path return if we already have the desired information.
        if self.seen_all_interferences || self.interfering_vregs.len() >= max_interfering_regs {
            return self.interfering_vregs.len();
        }
        // Set up iterators on the first call.
        if !self.checked_first_interference {
            self.checked_first_interference = true;
            // Quickly skip interference check for empty sets.
            if lr.empty() || union.empty() {
                self.seen_all_interferences = true;
                return 0;
            }
            // In most cases, the union will start before LR.
            self.lri = 0;
            self.live_union_i = union.find(&lr.segments[0].start);
        }
        let lr_end = lr.segments.len();
        let mut recent_reg: Option<VReg> = None;
        while let Some(key) = self.live_union_i.clone() {
            assert!(self.lri != lr_end, "Reached end of LR");
            let (mut ustart, mut ustop, mut ureg) = {
                let (stop, reg) = union.segments[&key].clone();
                (key, stop, reg)
            };
            // Check for overlapping interference.
            while lr.segments[self.lri].start < ustop && lr.segments[self.lri].end > ustart {
                // This is an overlap, record the interfering register.
                if recent_reg != Some(ureg) && !self.is_seen_interference(ureg) {
                    recent_reg = Some(ureg);
                    self.interfering_vregs.push(ureg);
                    if self.interfering_vregs.len() >= max_interfering_regs {
                        self.live_union_i = Some(ustart);
                        return self.interfering_vregs.len();
                    }
                }
                // This LiveUnion segment is no longer interesting.
                match union.next(&ustart) {
                    Some(next) => {
                        let (stop, reg) = union.segments[&next].clone();
                        ustart = next;
                        ustop = stop;
                        ureg = reg;
                    }
                    None => {
                        self.live_union_i = None;
                        self.seen_all_interferences = true;
                        return self.interfering_vregs.len();
                    }
                }
            }
            self.live_union_i = Some(ustart.clone());
            // The iterators are now not overlapping, LiveUnionI has been
            // advanced beyond LRI.
            assert!(lr.segments[self.lri].end <= ustart, "Expected non-overlap");
            // Advance the iterator that ends first.
            self.lri = lr.advance_to(self.lri, &ustart);
            if self.lri == lr_end {
                break;
            }
            // Detect overlap, handle above.
            if lr.segments[self.lri].start < ustop {
                continue;
            }
            // Still not overlapping. Catch up LiveUnionI.
            self.live_union_i = union.find(&lr.segments[self.lri].start);
        }
        self.seen_all_interferences = true;
        self.interfering_vregs.len()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use super::*;
    use crate::liveness::liverange::{LiveRange, Segment};
    use crate::liveness::slotindex::SlotIndexes;
    use crate::model::lir::{Block, Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn indexes() -> Vec<SlotIndex> {
        let insns: Vec<Arc<Insn>> = (0..300).map(|_| Arc::new(Insn::new(0, None, None, vec![], vec![]))).collect();
        let body = LirBody::new(
            "union",
            0,
            vec![Block::from(LirBlock::new(0, insns.clone()))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let indexes = SlotIndexes::new(&body);
        insns.iter().map(|one| indexes.get_instruction_index(one.id()).get_reg_slot(false)).collect()
    }

    fn range(
        spec: &[(usize, usize)],
        at: &[SlotIndex],
    ) -> LiveRange {
        let mut lr = LiveRange::new();
        let v = lr.get_next_value(at[spec[0].0].clone());
        for &(from, to) in spec {
            lr.add_segment(Segment::new(at[from].clone(), at[to].clone(), Some(v.clone())));
        }
        lr
    }

    struct Random(u64);

    impl Random {
        fn next(
            &mut self,
            n: usize,
        ) -> usize {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((self.0 >> 33) as usize) % n
        }
    }

    /// Disjoint segments for one register within `0..limit`.
    fn spec(
        random: &mut Random,
        limit: usize,
    ) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut p = random.next(10) + 1;
        for _ in 0..random.next(6) + 1 {
            let len = random.next(8) + 1;
            if p + len >= limit {
                break;
            }
            out.push((p, p + len));
            p += len + random.next(8);
        }
        if out.is_empty() { vec![(1, 2)] } else { out }
    }

    /// The registers a range interferes with, against the pairwise answer, for
    /// unions of several registers that do not overlap one another.
    #[test]
    fn test_the_registers_a_range_interferes_with_are_those_that_overlap_it() {
        let at = indexes();
        let mut random = Random(7);
        let mut hits = 0;
        for round in 0..300 {
            // registers take disjoint slices of 0..280 so the union has no
            // overlaps
            let mut union = LiveIntervalUnion::new();
            let mut held: Vec<(VReg, Vec<(usize, usize)>)> = Vec::new();
            let mut taken: Vec<bool> = vec![false; 300];
            for reg in 0..(random.next(5) + 1) as u32 {
                let s = spec(&mut random, 280);
                if s.iter().any(|&(a, b)| (a..b).any(|p| taken[p])) {
                    continue;
                }
                for &(a, b) in &s {
                    (a..b).for_each(|p| taken[p] = true);
                }
                union.unify(reg, &range(&s, &at));
                held.push((reg, s));
            }
            let probe = spec(&mut random, 280);
            let lr = range(&probe, &at);
            let want: BTreeSet<VReg> = held
                .iter()
                .filter(|(_, s)| s.iter().any(|x| probe.iter().any(|y| x.0 < y.1 && y.0 < x.1)))
                .map(|(r, _)| *r)
                .collect();
            let mut q = Query::new();
            q.init(1, round, 0, &union);
            assert_eq!(q.check_interference(&lr, &union), !want.is_empty(), "round {round}");
            let got: BTreeSet<VReg> = q.interfering_vregs(&lr, &union, usize::MAX).iter().copied().collect();
            assert_eq!(got, want, "round {round}: the first check and the full walk give the pairwise set");
            // cached: a second init with nothing changed retains the result
            q.init(1, round, 0, &union);
            assert_eq!(q.interfering_vregs(&lr, &union, usize::MAX).len(), want.len());
            hits += usize::from(!want.is_empty());
        }
        assert!(hits > 60 && hits < 299, "premise: some ranges interfere and some do not ({hits})");
    }

    #[test]
    fn test_a_query_stops_at_the_register_count_asked_for_and_goes_on_from_there() {
        let at = indexes();
        let mut union = LiveIntervalUnion::new();
        for (reg, from) in [(1u32, 10usize), (2, 20), (3, 30), (4, 40)] {
            union.unify(reg, &range(&[(from, from + 5)], &at));
        }
        let lr = range(&[(12, 50)], &at);
        let mut q = Query::new();
        q.init(1, 1, 0, &union);
        assert_eq!(q.collect_interfering_vregs(&lr, &union, 2), 2);
        assert_eq!(q.interfering_vregs(&lr, &union, usize::MAX), &[1, 2, 3, 4]);
    }

    /// A change to the union forgets what a query found (`changedSince`).
    #[test]
    fn test_a_changed_union_forgets_the_cached_answer() {
        let at = indexes();
        let mut union = LiveIntervalUnion::new();
        let lr = range(&[(10, 20)], &at);
        let mut q = Query::new();
        q.init(1, 1, 0, &union);
        assert!(!q.check_interference(&lr, &union));
        union.unify(9, &range(&[(12, 14)], &at));
        q.init(1, 1, 0, &union);
        assert!(q.check_interference(&lr, &union), "the union changed under the cached query");
    }

    #[test]
    fn test_adjacent_segments_of_a_register_are_one_entry_and_extract_removes_it() {
        let at = indexes();
        let mut union = LiveIntervalUnion::new();
        let mut lr = LiveRange::new();
        let (a, b) = (lr.get_next_value(at[10].clone()), lr.get_next_value(at[20].clone()));
        lr.add_segment(Segment::new(at[10].clone(), at[20].clone(), Some(a)));
        lr.add_segment(Segment::new(at[20].clone(), at[30].clone(), Some(b)));
        lr.add_segment(Segment::new(at[40].clone(), at[45].clone(), Some(lr.get_val_num_info(0))));
        assert_eq!(lr.segments.len(), 3, "different values stay apart in the range");
        union.unify(5, &lr);
        assert_eq!(union.iter().count(), 2, "{}", union.print());
        union.extract(5, &lr);
        assert!(union.empty(), "{}", union.print());
    }

    #[test]
    fn test_clear_all_segments_referencing_leaves_the_others() {
        let at = indexes();
        let mut union = LiveIntervalUnion::new();
        union.unify(1, &range(&[(10, 12)], &at));
        union.unify(2, &range(&[(20, 22)], &at));
        union.clear_all_segments_referencing(1);
        assert_eq!(union.get_one_vreg(), Some(2));
        assert_eq!(union.iter().count(), 1);
    }
}
