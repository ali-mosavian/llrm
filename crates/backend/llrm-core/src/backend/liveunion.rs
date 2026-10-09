//! What each register holds, by where: LLVM's `LiveIntervalUnion` and the
//! `LiveRegMatrix` over it.
//!
//! A register's holders do not overlap one another, so a query for what meets
//! an interval is a walk over the segments that start before it ends, back to
//! the first that ends before it starts: logarithmic and the overlaps found,
//! not the holders there are. A segment that does meet one already kept (a
//! rewrite moves every interval, and leaves the clashes for the allocator to
//! find) is kept apart and looked at one by one, so the answers are the
//! pairwise ones in every case.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};

use iced_x86::Register;

use crate::analysis::intervals::{Interval, Segment};
use crate::support::hash::IndexMap;

/// How many holders a register has before it is worth an index: fewer are
/// looked at one by one, as cheaply.
const BIG: usize = 48;
/// How many questions since the intervals last moved make an index worth
/// building: it costs about what a few dozen cost without.
const ASKED: u32 = 16;

/// What a register's holders are, by where they are live: segments that meet
/// none other kept (start to (end, holder)), points (a segment of no length
/// still meets another it lies inside), and every segment that meets one kept,
/// looked at one by one.
#[derive(Clone, Debug, Default)]
struct Index {
    /// Taking order to value, and back.
    by_taking: BTreeMap<u64, u32>,
    taken: IndexMap<u32, u64>,
    kept: BTreeMap<i64, (i64, u32)>,
    points: BTreeMap<i64, Vec<u32>>,
    apart: Vec<(i64, i64, u32)>,
}

impl Index {
    /// The holders that meet `[start, end)`, by when they took the register.
    fn meeting(
        &self,
        start: i64,
        end: i64,
        found: &mut BTreeSet<u64>,
    ) {
        for (_, (finish, holder)) in self.kept.range(..end).rev() {
            if *finish <= start {
                break;
            }
            found.insert(self.taken[holder]);
        }
        if start + 1 < end {
            for holders in self.points.range(start + 1..end).map(|(_, holders)| holders) {
                found.extend(holders.iter().map(|holder| self.taken[holder]));
            }
        }
        for (from, to, holder) in &self.apart {
            if *from < end && start < *to {
                found.insert(self.taken[holder]);
            }
        }
    }

    fn meets(
        &self,
        start: i64,
        end: i64,
    ) -> bool {
        if self.kept.range(..end).next_back().is_some_and(|(_, (finish, _))| *finish > start) {
            return true;
        }
        if start + 1 < end && self.points.range(start + 1..end).next().is_some() {
            return true;
        }
        self.apart.iter().any(|(from, to, _)| *from < end && start < *to)
    }

    fn keep(
        &mut self,
        segment: &Segment,
        value: u32,
    ) {
        if self.meets(segment.start, segment.end) {
            self.apart.push((segment.start, segment.end, value));
        } else if segment.end > segment.start {
            self.kept.insert(segment.start, (segment.end, value));
        } else {
            self.points.entry(segment.start).or_default().push(value);
        }
    }

    fn drop(
        &mut self,
        segment: &Segment,
        value: u32,
    ) {
        if let Some(at) = self.apart.iter().position(|one| *one == (segment.start, segment.end, value)) {
            self.apart.remove(at);
        } else if segment.end > segment.start {
            if self.kept.get(&segment.start) == Some(&(segment.end, value)) {
                self.kept.remove(&segment.start);
            }
        } else if let Some(holders) = self.points.get_mut(&segment.start) {
            holders.retain(|holder| *holder != value);
            if holders.is_empty() {
                self.points.remove(&segment.start);
            }
        }
    }

    fn add(
        &mut self,
        value: u32,
        at: u64,
        live: &IndexMap<u32, Interval>,
    ) {
        self.by_taking.insert(at, value);
        self.taken.insert(value, at);
        if let Some(interval) = live.get(&value) {
            for segment in &interval.segments {
                self.keep(segment, value);
            }
        }
    }

    fn remove(
        &mut self,
        value: u32,
        live: &IndexMap<u32, Interval>,
    ) {
        if let Some(at) = self.taken.shift_remove(&value) {
            self.by_taking.remove(&at);
            if let Some(interval) = live.get(&value) {
                for segment in &interval.segments {
                    self.drop(segment, value);
                }
            }
        }
    }
}

/// One register's holders in the order they took it, and an index of where they
/// are live once there are many.
#[derive(Clone, Debug, Default)]
struct Held {
    holders: Vec<u32>,
    index: RefCell<Option<Index>>,
}

/// The values each register holds, whole registers.
#[derive(Clone, Debug, Default)]
pub struct LiveUnion {
    registers: IndexMap<Register, Held>,
    taken: u64,
    /// Questions asked since the intervals last moved.
    asked: Cell<u32>,
}

impl LiveUnion {
    pub fn new() -> Self {
        Self::default()
    }

    /// `holders` as they stand (a test's, or a state to start from), where
    /// `live` has them.
    pub fn of(
        holders: impl IntoIterator<Item = (Register, Vec<u32>)>,
        live: &IndexMap<u32, Interval>,
    ) -> Self {
        let mut union = Self::new();
        for (register, values) in holders {
            for value in values {
                union.add(register, value, live);
            }
        }
        union
    }

    /// `value` takes `register`, for as long as `live` says it is live.
    pub fn add(
        &mut self,
        register: Register,
        value: u32,
        live: &IndexMap<u32, Interval>,
    ) {
        self.taken += 1;
        let at = self.taken;
        let held = self.registers.entry(register).or_default();
        held.holders.push(value);
        if let Some(index) = held.index.get_mut() {
            index.add(value, at, live);
        }
    }

    /// `value` gives up `register`: where it is live must be what it was when
    /// it took it.
    pub fn remove(
        &mut self,
        register: Register,
        value: u32,
        live: &IndexMap<u32, Interval>,
    ) {
        let held = self.registers.get_mut(&register).expect("a value is in the register it holds");
        held.holders.retain(|other| *other != value);
        if let Some(index) = held.index.get_mut() {
            index.remove(value, live);
        }
    }

    /// Every interval moved: the holders stay, and are asked about where `live`
    /// now has them.
    pub fn refresh(&mut self) {
        for held in self.registers.values_mut() {
            *held.index.get_mut() = None;
        }
        self.asked.set(0);
    }

    /// The holders of `register`, in the order they took it.
    pub fn holders(
        &self,
        register: &Register,
    ) -> Vec<u32> {
        self.registers.get(register).map(|held| held.holders.clone()).unwrap_or_default()
    }

    /// Each register with its holders.
    pub fn registers(&self) -> impl Iterator<Item = (Register, Vec<u32>)> + '_ {
        self.registers.iter().map(|(register, held)| (*register, held.holders.clone()))
    }

    /// `held`'s index, built once enough is asked of a register with many
    /// holders.
    fn indexed<'a>(
        &self,
        held: &'a Held,
        live: &IndexMap<u32, Interval>,
    ) -> Option<std::cell::Ref<'a, Index>> {
        self.asked.set(self.asked.get().saturating_add(1));
        if held.index.borrow().is_none() {
            if held.holders.len() < BIG || self.asked.get() < ASKED {
                return None;
            }
            let mut index = Index::default();
            for (at, value) in held.holders.iter().enumerate() {
                index.add(*value, at as u64 + 1, live);
            }
            *held.index.borrow_mut() = Some(index);
        }
        Some(std::cell::Ref::map(held.index.borrow(), |index| index.as_ref().expect("built")))
    }

    /// The holders of `register` that are live where `one` is, in the order
    /// they took it.
    pub fn meeting(
        &self,
        register: &Register,
        one: &Interval,
        live: &IndexMap<u32, Interval>,
    ) -> Vec<u32> {
        let Some(held) = self.registers.get(register) else { return Vec::new() };
        let Some(index) = self.indexed(held, live) else {
            return held
                .holders
                .iter()
                .copied()
                .filter(|other| live.get(other).is_some_and(|found| found.overlaps(one)))
                .collect();
        };
        let mut found = BTreeSet::new();
        for segment in &one.segments {
            index.meeting(segment.start, segment.end, &mut found);
        }
        found.into_iter().map(|at| index.by_taking[&at]).collect()
    }

    /// Whether any holder of `register` is live where `one` is.
    pub fn busy(
        &self,
        register: &Register,
        one: &Interval,
        live: &IndexMap<u32, Interval>,
    ) -> bool {
        let Some(held) = self.registers.get(register) else { return false };
        let Some(index) = self.indexed(held, live) else {
            return held.holders.iter().filter_map(|other| live.get(other)).any(|other| other.overlaps(one));
        };
        one.segments.iter().any(|segment| index.meets(segment.start, segment.end))
    }
}

/// Segments to be asked which meet an interval, none added or taken away: a
/// tree over the segments sorted by start, each node holding the furthest end
/// below it.
pub struct Overlaps {
    segments: Vec<(i64, i64, u32)>,
    furthest: Vec<i64>,
}

impl Overlaps {
    pub fn new(mut segments: Vec<(i64, i64, u32)>) -> Self {
        segments.sort_unstable();
        let mut furthest = vec![i64::MIN; segments.len()];
        Self::reach(&segments, &mut furthest, 0, segments.len());
        Self { segments, furthest }
    }

    fn reach(
        segments: &[(i64, i64, u32)],
        furthest: &mut [i64],
        low: usize,
        high: usize,
    ) -> i64 {
        if low >= high {
            return i64::MIN;
        }
        let middle = (low + high) / 2;
        let most = segments[middle]
            .1
            .max(Self::reach(segments, furthest, low, middle))
            .max(
                Self::reach(
                    segments,
                    furthest,
                    middle + 1,
                    high,
                ),
            );
        furthest[middle] = most;
        most
    }

    /// `each` of the values with a segment that meets `[start, end)`, once per
    /// segment that does.
    pub fn meeting(
        &self,
        start: i64,
        end: i64,
        each: &mut dyn FnMut(u32),
    ) {
        self.visit(0, self.segments.len(), start, end, each);
    }

    fn visit(
        &self,
        low: usize,
        high: usize,
        start: i64,
        end: i64,
        each: &mut dyn FnMut(u32),
    ) {
        if low >= high {
            return;
        }
        let middle = (low + high) / 2;
        if self.furthest[middle] <= start {
            return;
        }
        self.visit(low, middle, start, end, each);
        let (from, to, value) = self.segments[middle];
        if from < end {
            if to > start {
                each(value);
            }
            self.visit(middle + 1, high, start, end, each);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interval(
        value: u32,
        segments: &[(i64, i64)],
    ) -> Interval {
        Interval::new(value, segments.iter().map(|(start, end)| Segment { start: *start, end: *end }).collect())
    }

    fn live(all: &[(u32, &[(i64, i64)])]) -> IndexMap<u32, Interval> {
        all.iter().map(|(value, segments)| (*value, interval(*value, segments))).collect()
    }

    /// The pairwise look: what the holders' list was asked for.
    fn pairwise(
        holders: &[u32],
        live: &IndexMap<u32, Interval>,
        one: &Interval,
    ) -> Vec<u32> {
        holders.iter().copied().filter(|other| live.get(other).is_some_and(|found| found.overlaps(one))).collect()
    }

    /// Enough questions of a register with enough holders to have it indexed:
    /// the answers are the pairwise ones.
    fn asked_of_many(
        live: &IndexMap<u32, Interval>,
        taking: &[u32],
        queries: &[(i64, i64)],
    ) {
        let mut union = LiveUnion::new();
        for value in taking {
            union.add(Register::AX, *value, live);
        }
        let holders = union.holders(&Register::AX);
        assert_eq!(&holders, taking);
        for round in 0..3 {
            for query in queries {
                let one = interval(0, &[*query]);
                assert_eq!(
                    union.meeting(&Register::AX, &one, live),
                    pairwise(&holders, live, &one),
                    "{query:?} round {round}"
                );
                assert_eq!(
                    union.busy(&Register::AX, &one, live),
                    !pairwise(&holders, live, &one).is_empty(),
                    "{query:?} round {round}"
                );
            }
        }
    }

    #[test]
    fn test_holders_meeting_an_interval_are_those_the_pairwise_look_finds_in_the_order_they_took_it() {
        let live = live(&[
            (1, &[(10, 20)]),
            (2, &[(0, 5), (30, 40)]),
            (3, &[(20, 30)]),
            (4, &[(7, 7)]),
            (5, &[(12, 12)]),
            (6, &[(15, 35)]),
        ]);
        let queries = [
            (0, 100),
            (5, 10),
            (12, 13),
            (14, 16),
            (19, 21),
            (30, 31),
            (7, 8),
            (6, 8),
            (11, 12),
            (50, 60),
            (12, 12),
            (16, 16),
        ];
        asked_of_many(&live, &[3, 1, 2, 4, 5, 6], &queries);
    }

    #[test]
    fn test_a_register_with_many_holders_is_indexed_and_answers_as_the_pairwise_look_does() {
        // 100 holders, some clashing (the intervals of a rewrite), points among
        // them.
        let mut all: Vec<(u32, Vec<(i64, i64)>)> = Vec::new();
        for value in 0..100u32 {
            let start = i64::from(value) * 10;
            let mut segments = vec![(start, start + 7)];
            if value % 9 == 0 {
                segments.push((start + 8, start + 8));
            }
            if value % 13 == 0 {
                segments.push((start + 3, start + 25));
            }
            all.push((value + 1, segments));
        }
        let live: IndexMap<u32, Interval> =
            all.iter().map(|(value, segments)| (*value, interval(*value, segments))).collect();
        let taking: Vec<u32> = (1..=100).rev().collect();
        let queries: Vec<(i64, i64)> =
            (0..120).map(|k| (i64::from(k) * 9, i64::from(k) * 9 + 1 + i64::from(k % 5) * 6)).collect();
        asked_of_many(&live, &taking, &queries);
    }

    #[test]
    fn test_a_holder_taken_away_meets_nothing_and_one_taken_again_comes_last() {
        let live = live(&[(1, &[(0, 10)]), (2, &[(10, 20)]), (3, &[(20, 30)])]);
        let mut union = LiveUnion::new();
        for value in [1, 2, 3] {
            union.add(Register::BX, value, &live);
        }
        union.remove(Register::BX, 2, &live);
        assert!(!union.busy(&Register::BX, &interval(0, &[(10, 20)]), &live));
        union.add(Register::BX, 2, &live);
        assert_eq!(union.holders(&Register::BX), vec![1, 3, 2]);
        assert_eq!(union.meeting(&Register::BX, &interval(0, &[(5, 25)]), &live), vec![1, 3, 2]);
    }

    #[test]
    fn test_holders_that_clash_after_the_intervals_move_are_all_found_until_one_is_removed() {
        let before = live(&[(1, &[(0, 10)]), (2, &[(10, 20)])]);
        let mut union = LiveUnion::new();
        union.add(Register::CX, 1, &before);
        union.add(Register::CX, 2, &before);
        // A rewrite moves both over each other.
        let after = live(&[(1, &[(0, 15)]), (2, &[(5, 20)])]);
        union.refresh();
        assert_eq!(union.meeting(&Register::CX, &interval(0, &[(12, 13)]), &after), vec![1, 2]);
        union.remove(Register::CX, 2, &after);
        assert_eq!(union.meeting(&Register::CX, &interval(0, &[(12, 13)]), &after), vec![1]);
        assert!(!union.busy(&Register::CX, &interval(0, &[(16, 18)]), &after));
    }

    #[test]
    fn test_overlaps_finds_each_segment_that_meets_a_range() {
        let tree = Overlaps::new(vec![(0, 10, 1), (5, 8, 2), (9, 30, 3), (40, 50, 4), (45, 45, 5), (60, 70, 6)]);
        let ask = |start: i64, end: i64| {
            let mut found = Vec::new();
            tree.meeting(start, end, &mut |value| found.push(value));
            found.sort_unstable();
            found
        };
        assert_eq!(ask(6, 9), vec![1, 2]);
        assert_eq!(ask(8, 10), vec![1, 3]);
        assert_eq!(ask(44, 46), vec![4, 5]);
        assert_eq!(ask(31, 39), Vec::<u32>::new());
        assert_eq!(ask(0, 100), vec![1, 2, 3, 4, 5, 6]);
    }
}
