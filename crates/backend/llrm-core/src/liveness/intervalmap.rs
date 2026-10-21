//! `IntervalMap` as `LiveIntervalUnion` uses it: disjoint entries `[start,
//! stop) -> value` in start order, in leaves of at most [`LEAF`] entries
//! (llvm/include/llvm/ADT/IntervalMap.h at d1106deb71cc, a B+ tree with the
//! leaves in a row).
//!
//! DEVIATE: one level of leaves in a vector, not a tree. The fact: a union
//! holds the segments of one register unit, hundreds to a few thousand; a leaf
//! split moves [`LEAF`] entries and the row of leaves is searched by binary
//! search.
//!
//! A [`Pos`] is an iterator: `next` and `prev` step in O(1) and `at` reads the
//! entry. It stays valid until the map is changed.
//!
//! # Used by
//! `LiveIntervalUnion`.

use super::slotindex::SlotIndex;

/// Entries in a leaf before it splits in two.
const LEAF: usize = 64;

#[cfg(test)]
thread_local! {
    static SEARCHES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The searches by key made on this thread, for a test that a cursor walking
/// blocks does not search per step.
#[cfg(test)]
pub fn searches() -> usize {
    SEARCHES.with(std::cell::Cell::get)
}

#[cfg(test)]
thread_local! {
    static COMPARES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The stops `find` has compared on this thread, for a test that one find is
/// one search.
#[cfg(test)]
pub fn compares() -> usize {
    COMPARES.with(std::cell::Cell::get)
}

/// One entry: `[start, stop)` held by `value`.
#[derive(Clone, Copy)]
pub struct Entry<V> {
    pub start: SlotIndex,
    pub stop: SlotIndex,
    pub value: V,
}

/// An iterator: the leaf and the place in it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pos {
    leaf: u32,
    at: u32,
}

#[derive(Default)]
pub struct IntervalMap<V> {
    leaves: Vec<Vec<Entry<V>>>,
}

impl<V: Copy> IntervalMap<V> {
    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }

    pub fn clear(&mut self) {
        self.leaves.clear();
    }

    pub fn first(&self) -> Option<&Entry<V>> {
        self.leaves.first().and_then(|leaf| leaf.first())
    }

    pub fn last(&self) -> Option<&Entry<V>> {
        self.leaves.last().and_then(|leaf| leaf.last())
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entry<V>> {
        self.leaves.iter().flatten()
    }

    /// `I.start()`, `I.stop()` and `I.value()`.
    pub fn at(
        &self,
        pos: Pos,
    ) -> &Entry<V> {
        &self.leaves[pos.leaf as usize][pos.at as usize]
    }

    /// `++I`: None at the end.
    pub fn next(
        &self,
        pos: Pos,
    ) -> Option<Pos> {
        if (pos.at as usize) + 1 < self.leaves[pos.leaf as usize].len() {
            return Some(Pos { leaf: pos.leaf, at: pos.at + 1 });
        }
        ((pos.leaf as usize) + 1 < self.leaves.len()).then_some(Pos { leaf: pos.leaf + 1, at: 0 })
    }

    /// `--I`; from the end (None), the last entry.
    pub fn prev(
        &self,
        pos: Option<Pos>,
    ) -> Option<Pos> {
        let Some(pos) = pos else {
            let leaf = self.leaves.len().checked_sub(1)?;
            return Some(Pos { leaf: leaf as u32, at: (self.leaves[leaf].len() - 1) as u32 });
        };
        if pos.at > 0 {
            return Some(Pos { leaf: pos.leaf, at: pos.at - 1 });
        }
        let leaf = (pos.leaf as usize).checked_sub(1)?;
        Some(Pos { leaf: leaf as u32, at: (self.leaves[leaf].len() - 1) as u32 })
    }

    /// The first entry whose start is not before `key`, as an insertion place:
    /// past the last entry of the last leaf at the end.
    fn lower_bound(
        &self,
        key: &SlotIndex,
    ) -> (usize, usize) {
        #[cfg(test)]
        SEARCHES.with(|n| n.set(n.get() + 1));
        let leaf = self.leaves.partition_point(|leaf| leaf.last().is_some_and(|e| e.start < *key));
        if leaf == self.leaves.len() {
            return (leaf.saturating_sub(1), self.leaves.last().map_or(0, Vec::len));
        }
        (leaf, self.leaves[leaf].partition_point(|e| e.start < *key))
    }

    /// The place `lower_bound` names, as an iterator; None at the end.
    fn pos_of(
        &self,
        (leaf, at): (usize, usize),
    ) -> Option<Pos> {
        (leaf < self.leaves.len() && at < self.leaves[leaf].len()).then_some(Pos { leaf: leaf as u32, at: at as u32 })
    }

    /// The entry starting at `key`.
    pub fn get(
        &self,
        key: &SlotIndex,
    ) -> Option<Pos> {
        self.pos_of(self.lower_bound(key)).filter(|pos| self.at(*pos).start == *key)
    }

    /// The entry starting last before `key`.
    pub fn before(
        &self,
        key: &SlotIndex,
    ) -> Option<Pos> {
        let (leaf, at) = self.lower_bound(key);
        if leaf >= self.leaves.len() {
            return None;
        }
        if at < self.leaves[leaf].len() {
            return self.prev(Some(Pos { leaf: leaf as u32, at: at as u32 }));
        }
        self.prev(None)
    }

    /// `find(x)`: the first entry whose stop is after `x`, the one holding it
    /// or the first after the hole it is in (one search on the stops, as
    /// `IntervalMap::find`'s `safeFind(stopLess)`; the entries are
    /// disjoint, so their stops are in order too).
    pub fn find(
        &self,
        x: &SlotIndex,
    ) -> Option<Pos> {
        let stops_by = |entry: &Entry<V>| {
            #[cfg(test)]
            COMPARES.with(|n| n.set(n.get() + 1));
            entry.stop <= *x
        };
        let leaf = self.leaves.partition_point(|leaf| leaf.last().is_some_and(stops_by));
        let at = self.leaves.get(leaf)?.partition_point(stops_by);
        Some(Pos { leaf: leaf as u32, at: at as u32 })
    }

    /// `advanceTo(x)`: the first entry at or after `pos` whose stop is after
    /// `x`: a few steps, then a search.
    pub fn advance_to(
        &self,
        pos: Option<Pos>,
        x: &SlotIndex,
    ) -> Option<Pos> {
        let mut pos = pos?;
        for _ in 0..4 {
            if self.at(pos).stop > *x {
                return Some(pos);
            }
            pos = self.next(pos)?;
        }
        self.find(x)
    }

    /// `LiveIntervalUnion::unify`'s loop
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveIntervalUnion.cpp#L28>):
    /// the `[start, stop)` of `run`, ascending and disjoint, held by `value`,
    /// each joined to an adjacent entry of the same value as `IntervalMap::
    /// insert` does. One search finds the place of the first; each next entry
    /// is looked for from the one before (`SegPos.advanceTo`), a few steps and
    /// then a search.
    pub fn insert_run(
        &mut self,
        run: impl IntoIterator<Item = (SlotIndex, SlotIndex)>,
        value: V,
    ) where
        V: PartialEq,
    {
        let mut run = run.into_iter();
        let Some((mut start, mut stop)) = run.next() else { return };
        let mut cursor = self.place_of(self.lower_bound(&start));
        loop {
            // The first entry from the cursor on that starts at or after
            // `start`.
            let mut steps = 0;
            while let Some(entry) = self.entry_at(cursor) {
                if entry.start >= start {
                    break;
                }
                steps += 1;
                if steps > 4 {
                    cursor = self.place_of(self.lower_bound(&start));
                    break;
                }
                cursor = self.step(cursor);
            }
            // Join the entry before it, then the entry at it.
            if let Some(before) = self.back(cursor) {
                let left = *self.entry_at(Some(before)).expect("an entry");
                if left.stop == start && left.value == value {
                    start = left.start;
                    self.remove_at(before);
                    cursor = self.fix_after_removal(before);
                }
            }
            if let Some(right) = self.entry_at(cursor).copied()
                && right.start == stop
                && right.value == value
            {
                stop = right.stop;
                self.remove_at(cursor.expect("an entry"));
                cursor = self.fix_after_removal(cursor.expect("an entry"));
            }
            cursor = Some(self.insert_before(cursor, Entry { start, stop, value }));
            cursor = self.step(cursor);
            let Some((next_start, next_stop)) = run.next() else { return };
            (start, stop) = (next_start, next_stop);
        }
    }

    /// `lower_bound`'s answer as a place: past the end of a leaf is the start
    /// of the next, and past the last entry is `None`.
    fn place_of(
        &self,
        (leaf, at): (usize, usize),
    ) -> Option<(usize, usize)> {
        let len = self.leaves.get(leaf)?.len();
        if at < len {
            return Some((leaf, at));
        }
        (leaf + 1 < self.leaves.len()).then_some((leaf + 1, 0))
    }

    /// The entry at a raw place, None past the end.
    fn entry_at(
        &self,
        place: Option<(usize, usize)>,
    ) -> Option<&Entry<V>> {
        let (leaf, at) = place?;
        self.leaves.get(leaf)?.get(at)
    }

    /// The place after `place`; the end is `None`.
    fn step(
        &self,
        place: Option<(usize, usize)>,
    ) -> Option<(usize, usize)> {
        let (leaf, at) = place?;
        if at + 1 < self.leaves[leaf].len() {
            return Some((leaf, at + 1));
        }
        (leaf + 1 < self.leaves.len()).then_some((leaf + 1, 0))
    }

    /// The place before `place`; before the first is `None`. From the end,
    /// the last entry.
    fn back(
        &self,
        place: Option<(usize, usize)>,
    ) -> Option<(usize, usize)> {
        match place {
            Some((leaf, at)) if at > 0 => Some((leaf, at - 1)),
            Some((leaf, _)) => {
                let leaf = leaf.checked_sub(1)?;
                Some((leaf, self.leaves[leaf].len() - 1))
            }
            None => {
                let leaf = self.leaves.len().checked_sub(1)?;
                Some((leaf, self.leaves[leaf].len() - 1))
            }
        }
    }

    fn remove_at(
        &mut self,
        (leaf, at): (usize, usize),
    ) {
        self.leaves[leaf].remove(at);
        if self.leaves[leaf].is_empty() {
            self.leaves.remove(leaf);
        }
    }

    /// The place of the entry that followed the one removed at `removed`.
    fn fix_after_removal(
        &self,
        removed: (usize, usize),
    ) -> Option<(usize, usize)> {
        let (leaf, at) = removed;
        if leaf < self.leaves.len() && at < self.leaves[leaf].len() {
            return Some((leaf, at));
        }
        let next = if leaf < self.leaves.len() { leaf + 1 } else { leaf };
        (next < self.leaves.len()).then_some((next, 0))
    }

    /// Puts `entry` before the entry at `place` (at the end for `None`), splits
    /// a leaf that is too full, and returns where the entry is.
    fn insert_before(
        &mut self,
        place: Option<(usize, usize)>,
        entry: Entry<V>,
    ) -> (usize, usize) {
        if self.leaves.is_empty() {
            self.leaves.push(vec![entry]);
            return (0, 0);
        }
        let (leaf, at) = place.unwrap_or_else(|| {
            let last = self.leaves.len() - 1;
            (last, self.leaves[last].len())
        });
        self.leaves[leaf].insert(at, entry);
        if self.leaves[leaf].len() > LEAF {
            let tail = self.leaves[leaf].split_off(LEAF / 2);
            self.leaves.insert(leaf + 1, tail);
            if at >= LEAF / 2 {
                return (leaf + 1, at - LEAF / 2);
            }
        }
        (leaf, at)
    }

    /// Adds an entry; its start is not in the map.
    pub fn insert(
        &mut self,
        entry: Entry<V>,
    ) {
        if self.leaves.is_empty() {
            self.leaves.push(vec![entry]);
            return;
        }
        let (leaf, at) = self.lower_bound(&entry.start);
        self.leaves[leaf].insert(at, entry);
        if self.leaves[leaf].len() > LEAF {
            let tail = self.leaves[leaf].split_off(LEAF / 2);
            self.leaves.insert(leaf + 1, tail);
        }
    }

    /// Removes the entry at `pos`; the iterator that was at it is `pos_after`
    /// of the result.
    pub fn remove(
        &mut self,
        pos: Pos,
    ) -> Entry<V> {
        let leaf = pos.leaf as usize;
        let entry = self.leaves[leaf].remove(pos.at as usize);
        if self.leaves[leaf].is_empty() {
            self.leaves.remove(leaf);
        }
        entry
    }

    /// Where the iterator at a removed entry is now: the entry after it.
    pub fn after_removed(
        &self,
        pos: Pos,
    ) -> Option<Pos> {
        let leaf = pos.leaf as usize;
        if leaf < self.leaves.len() && (pos.at as usize) < self.leaves[leaf].len() {
            return Some(pos);
        }
        // The leaf is gone (its next is here) or the entry was its last (the
        // next leaf's first).
        let next =
            if leaf < self.leaves.len() && (pos.at as usize) >= self.leaves[leaf].len() { leaf + 1 } else { leaf };
        (next < self.leaves.len()).then_some(Pos { leaf: next as u32, at: 0 })
    }

    pub fn retain(
        &mut self,
        mut keep: impl FnMut(&Entry<V>) -> bool,
    ) {
        for leaf in &mut self.leaves {
            leaf.retain(|e| keep(e));
        }
        self.leaves.retain(|leaf| !leaf.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use super::*;
    use crate::liveness::slotindex::SlotIndexes;
    use crate::model::lir::{Block, Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn indexes() -> Vec<SlotIndex> {
        let insns: Vec<Arc<Insn>> = (0..600).map(|_| Arc::new(Insn::new(0, None, None, vec![], vec![]))).collect();
        let body = LirBody::new(
            "m",
            0,
            vec![Block::from(LirBlock::new(0, insns.clone()))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let indexes = SlotIndexes::new(&body);
        insns.iter().map(|i| indexes.get_instruction_index(i.id()).get_reg_slot(false)).collect()
    }

    /// Entries added and removed in a fixed order that splits and empties
    /// leaves answer `find`, `before`, `next` and `prev` as a `BTreeMap` of
    /// the same entries does.
    #[test]
    fn test_a_row_of_leaves_answers_as_a_btree_map_does() {
        let at = indexes();
        let mut map = IntervalMap::<u32>::default();
        let mut model = BTreeMap::new();
        for step in 0..3000 {
            let i = (step * 37 % 299) * 2;
            if model.contains_key(&at[i]) && step % 3 != 0 {
                let pos = map.get(&at[i]).expect("the entry");
                map.remove(pos);
                model.remove(&at[i]);
            } else if !model.contains_key(&at[i]) {
                map.insert(Entry { start: at[i], stop: at[i + 1], value: i as u32 });
                model.insert(at[i], (at[i + 1], i as u32));
            }
            let x = at[step * 101 % 599];
            let want = model
                .range(..=x)
                .next_back()
                .filter(|(_, (stop, _))| *stop > x)
                .or_else(|| model.range((std::ops::Bound::Excluded(x), std::ops::Bound::Unbounded)).next())
                .map(|(start, _)| *start);
            let got = map.find(&x);
            assert_eq!(got.map(|p| map.at(p).start), want, "find at step {step}");
            assert_eq!(
                map.before(&x).map(|p| map.at(p).start),
                model.range(..x).next_back().map(|(s, _)| *s),
                "before at {step}"
            );
            if let Some(p) = got {
                assert_eq!(
                    map.next(p).map(|p| map.at(p).start),
                    model
                        .range((std::ops::Bound::Excluded(map.at(p).start), std::ops::Bound::Unbounded))
                        .next()
                        .map(|(s, _)| *s)
                );
                assert_eq!(
                    map.prev(Some(p)).map(|p| map.at(p).start),
                    model.range(..map.at(p).start).next_back().map(|(s, _)| *s)
                );
            }
        }
        assert_eq!(map.iter().map(|e| e.start).collect::<Vec<_>>(), model.keys().copied().collect::<Vec<_>>());
        assert_eq!(map.prev(None).map(|p| map.at(p).start), model.keys().next_back().copied());
    }

    /// `find` was a search on the starts, then a step back and two compares
    /// more (333 instructions a call on d_faces); it is one search on the
    /// stops: the leaf, then the entry in it.
    #[test]
    fn test_a_find_is_one_search_on_the_stops() {
        let at = indexes();
        let mut map = IntervalMap::<u32>::default();
        for i in 0..250 {
            map.insert(Entry { start: at[2 * i], stop: at[2 * i + 1], value: i as u32 });
        }
        let leaves = map.leaves.len();
        let bound =
            (usize::BITS - leaves.leading_zeros()) as usize + (usize::BITS - (LEAF + 1).leading_zeros()) as usize;
        for x in [at[0], at[301], at[302], at[499]] {
            let (before, starts) = (compares(), searches());
            let found = map.find(&x);
            let spent = compares() - before;
            assert_eq!(searches() - starts, 0, "a find searched the starts");
            assert!(spent <= bound, "{spent} compares to find ({bound} allowed for {leaves} leaves)");
            assert_eq!(found.map(|p| map.at(p).stop > x), Some(true).filter(|_| found.is_some()));
        }
        assert!(map.find(&at[501]).is_none(), "past the last stop");
    }
}
