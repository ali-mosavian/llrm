//! Port of `qbopt/analysis/intervals.py`: where each value is live, as
//! ranges rather than as sets per block.
//!
//! LLVM's `LiveIntervals`, two slots per instruction: where it reads, and
//! where it writes.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::analysis::frequency::Frequency;
use crate::support::hash::{IndexMap, IndexSet};

use crate::analysis::loops as loopy;
use crate::backend::allocate;
use crate::model::lir::{Insn, Insns, LirBlock, LirBody};

pub const DEF: i64 = 1;
pub const PER_INSN: i64 = 2;

/// A half-open run of slot indices one value occupies.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Segment {
    pub start: i64,
    pub end: i64,
}

impl Segment {
    pub fn overlaps(&self, other: &Segment) -> bool {
        self.start < other.end && other.start < self.end
    }
}

/// Every segment one value occupies, and what spilling it would cost.
#[derive(Clone, Debug, PartialEq)]
pub struct Interval {
    pub value: u32,
    pub segments: Vec<Segment>,
    pub weight: f64,
}

impl Interval {
    pub fn new(value: u32, segments: Vec<Segment>) -> Self {
        Self { value, segments, weight: 0.0 }
    }

    /// How many slots this value is live for, which is not its span.
    pub fn size(&self) -> i64 {
        self.segments.iter().map(|one| one.end - one.start).sum()
    }

    /// What spilling this value is divided by: how many slots it is live for, and the grace every value has.
    pub fn spill_size(&self) -> i64 {
        self.size() + GRACE
    }

    pub fn overlaps(&self, other: &Interval) -> bool {
        let (mut mine, mut theirs) = (self.segments.iter(), other.segments.iter());
        let (mut one, mut two) = (mine.next(), theirs.next());
        while let (Some(first), Some(second)) = (one, two) {
            if first.overlaps(second) {
                return true;
            }
            if first.end <= second.end {
                one = mine.next();
            } else {
                two = theirs.next();
            }
        }
        false
    }
}

/// Python `id(insn)`: the instruction's identity in the body that holds it.
pub fn key(one: &Arc<Insn>) -> usize {
    Arc::as_ptr(one) as usize
}

/// Every instruction's slot number, and every block's span.
#[derive(Clone, Debug)]
pub struct Indexes {
    pub at: IndexMap<usize, i64>, // id(insn) -> the instruction's first slot
    pub span: IndexMap<i64, (i64, i64)>, // block address -> [first, last)
    pub order: Vec<i64>,          // block addresses, in the order they are numbered
}

/// Where an instruction writes, given where it reads: the second of the two slots it holds.
pub fn def_point(slot: i64) -> i64 {
    slot + DEF
}

impl Indexes {
    /// Where the instruction at `position` in `block` ends: the point its two slots end at, which is where the next
    /// instruction (or the block's end) begins. A segment that ends here touches one that starts at the next.
    pub fn window_end(&self, block: &LirBlock, position: usize) -> i64 {
        self.slot(block, position) + PER_INSN
    }

    /// The slot of the instruction at `position` in `block`, or the block's end past its last.
    pub fn slot(&self, block: &LirBlock, position: usize) -> i64 {
        block.insns.get(position).map_or(self.span[&block.at].1, |one| self.at[&key(one)])
    }

    /// The position in `block` of the instruction whose slots hold `slot`: -1
    /// for the block's entry, its length for its end.
    pub fn position(&self, block: &LirBlock, slot: i64) -> i64 {
        let (first, last) = self.span[&block.at];
        if slot < first + PER_INSN {
            return -1;
        }
        if slot >= last {
            return block.insns.len() as i64;
        }
        block.insns.iter().rposition(|one| self.at[&key(one)] <= slot).map_or(-1, |at| at as i64)
    }
}

/// Number every point a value can start or stop being live.
pub fn indexed(body: &LirBody) -> Indexes {
    // Sized for what it holds: growing a table of a function's instructions by doubling rehashed it six times over.
    let mut at = IndexMap::with_capacity_and_hasher(body.blocks.iter().map(|block| block.insns.len()).sum(), Default::default());
    let mut span = IndexMap::with_capacity_and_hasher(body.blocks.len(), Default::default());
    let mut next_slot = 0;
    for block in &body.blocks {
        let first = next_slot;
        // A phi's result is defined before the block's first instruction.
        next_slot += PER_INSN;
        for one in &block.insns {
            at.insert(key(one), next_slot);
            // A meta instruction takes no slot, as LLVM's SlotIndexes skip
            // debug instructions: a range across one is no longer.
            if !one.is_meta() {
                next_slot += PER_INSN;
            }
        }
        span.insert(block.at, (first, next_slot));
    }
    Indexes { at, span, order: body.blocks.iter().map(|block| block.at).collect() }
}

/// The numberings made, most recent first, each with the blocks' instructions it was of (held): the bodies a function's allocator
/// alternates among each find theirs.
#[derive(Default)]
struct Numbered(Vec<(Vec<(i64, Insns)>, Arc<Indexes>)>);

/// How many times this body's facts have numbered it for `indexed_shared`, for a test that asking again of one body does not.
pub fn numbered(body: &LirBody) -> usize {
    body.facts.0.counted("indexed")
}

/// `indexed(body)`, remembered for the next ask of the same instructions (block by block, by identity, in the same order).
pub fn indexed_shared(body: &LirBody) -> Arc<Indexes> {
    let kept = body.facts.0.stash(|held: &mut Numbered| {
        let at = held.0.iter().position(|(blocks, _)| blocks.len() == body.blocks.len() && blocks.iter().zip(&body.blocks).all(|((at, insns), block)| *at == block.at && insns.same_insns(&block.insns)))?;
        let found = held.0.remove(at);
        let index = Arc::clone(&found.1);
        held.0.insert(0, found);
        Some(index)
    });
    if let Some(found) = kept {
        return found;
    }
    body.facts.0.bump("indexed");
    let found = Arc::new(indexed(body));
    body.facts.0.stash(|held: &mut Numbered| {
        held.0.insert(0, (body.blocks.iter().map(|block| (block.at, block.insns.clone())).collect(), Arc::clone(&found)));
        held.0.truncate(REMEMBERED);
    });
    found
}

/// What an answer of `intervals_over` was made of: the body's blocks, each by the instructions it held (shared, so that an address is
/// not reused while it is remembered, and a block that is the same allocation needs no looking at), its phis, and what its
/// frequencies read.
struct Remembered {
    entry: i64,
    blocks: Vec<(i64, Vec<i64>, Vec<crate::model::lir::Phi>, Insns)>,
    count: usize,
    odds: crate::model::lir::BlockOdds,
    trips: Vec<(i64, i64)>,
    answer: Arc<IndexMap<u32, Interval>>,
    /// The body's numbering, and what each value's references weigh before they are divided by its size.
    index: Arc<Indexes>,
    totals: Arc<IndexMap<u32, f64>>,
}

impl Remembered {
    fn of(body: &LirBody, answer: &Arc<IndexMap<u32, Interval>>, index: &Arc<Indexes>, totals: IndexMap<u32, f64>) -> Self {
        Self {
            entry: body.entry,
            blocks: body.blocks.iter().map(|block| (block.at, block.succ.clone(), block.phis.clone(), block.insns.clone())).collect(),
            count: body.blocks.iter().map(|block| block.insns.len()).sum(),
            odds: body.odds.clone(),
            trips: body.loop_trip_counts.clone(),
            answer: Arc::clone(answer),
            index: Arc::clone(index),
            totals: Arc::new(totals),
        }
    }

    fn is_of(&self, body: &LirBody) -> bool {
        self.entry == body.entry
            && self.blocks.len() == body.blocks.len()
            && self.blocks.iter().zip(&body.blocks).all(|((at, succ, phis, insns), block)| *at == block.at && *succ == block.succ && *phis == block.phis && insns.same_insns(&block.insns))
            && self.odds == body.odds
            && self.trips == body.loop_trip_counts
    }
}

/// Where the slots of an earlier numbering are in a later one, for a body that keeps most of the earlier one's blocks: the block
/// tops carry the instructions of the blocks that are the same (each is where it was, relative to its top), and each instruction
/// both have in the others is a point of its own.
pub struct Shift {
    old: Vec<i64>,
    new: Vec<i64>,
    tops: crate::support::hash::HashMap<i64, i64>,
}

/// The blocks of `body` that are not the same instructions as `held`'s (by position).
pub fn differing_blocks(held: &[(i64, &Insns)], body: &LirBody) -> Vec<usize> {
    (0..body.blocks.len()).filter(|at| !held[*at].1.same_insns(&body.blocks[*at].insns)).collect()
}

/// The runs of instructions in only one of the two bodies, and of the parallel copy each is in (a copy's moves are all read and
/// written at its last, which a change to any of them moves), each run once; the number of instructions that changed.
pub fn changed_runs(held: &[(i64, &Insns)], held_index: &Indexes, body: &LirBody, index: &Indexes, differing: &[usize], visit: &mut dyn FnMut(&[&Arc<Insn>])) -> usize {
    let mut changed = 0;
    let mut runs = |run: &[&Arc<Insn>], gone: &dyn Fn(&Arc<Insn>) -> bool| {
        let mut start = 0;
        while start < run.len() {
            let mut end = start + 1;
            if run[start].group.is_some() {
                while end < run.len() && run[end].group == run[start].group {
                    end += 1;
                }
            }
            let hit = run[start..end].iter().filter(|one| gone(one)).count();
            if hit > 0 {
                changed += hit;
                visit(&run[start..end]);
            }
            start = end;
        }
    };
    for at in differing {
        let run: Vec<&Arc<Insn>> = held[*at].1.iter().collect();
        runs(&run, &|one| !index.at.contains_key(&key(one)));
        let run: Vec<&Arc<Insn>> = body.blocks[*at].insns.iter().collect();
        runs(&run, &|one| !held_index.at.contains_key(&key(one)));
    }
    changed
}

impl Shift {
    pub fn between(held: &[(i64, &Insns)], held_index: &Indexes, body: &LirBody, index: &Indexes, differing: &[usize]) -> Self {
        let (mut old, mut new): (Vec<i64>, Vec<i64>) = (Vec::new(), Vec::new());
        let mut tops: crate::support::hash::HashMap<i64, i64> = Default::default();
        for (position, ((at, insns), block)) in held.iter().zip(&body.blocks).enumerate() {
            let (before, now) = (held_index.span[at], index.span[&block.at]);
            old.push(before.0);
            new.push(now.0);
            tops.insert(before.0, now.0);
            tops.insert(before.1, now.1);
            if differing.binary_search(&position).is_ok() {
                for one in insns.iter() {
                    if let Some(slot) = index.at.get(&key(one)) {
                        old.push(held_index.at[&key(one)]);
                        new.push(*slot);
                    }
                }
            }
        }
        Self { old, new, tops }
    }

    /// Where a segment's start was.
    pub fn start(&self, slot: i64) -> i64 {
        if let Some(top) = self.tops.get(&slot) {
            return *top;
        }
        let at = self.old.partition_point(|one| *one <= slot) - 1;
        self.new[at] + (slot - self.old[at])
    }

    /// Where a segment's end was.
    pub fn end(&self, slot: i64) -> i64 {
        if let Some(top) = self.tops.get(&slot) {
            return *top;
        }
        let at = self.old.partition_point(|one| *one < slot) - 1;
        self.new[at] + (slot - self.old[at])
    }
}

/// The answers remembered for this function's bodies, most recent first: the allocator's rewrites and its trial candidates alternate
/// among a few, and they share the manager, so each is found by its blocks whichever asked last.
#[derive(Default)]
struct Recent(Vec<Arc<Remembered>>);

/// How many times this body's facts have worked out intervals, for a test that asking again of one body does not.
pub fn worked(body: &LirBody) -> usize {
    body.facts.0.counted("intervals-worked")
}

/// How many edited answers this body's facts have worked the changed values of out from where they occur, not by walking the body.
pub fn by_occurrences(body: &LirBody) -> usize {
    body.facts.0.counted("intervals-by-occurrences")
}

/// How many answers this body's facts have made by editing an earlier one, for a test that a body made of another's instructions is.
pub fn edited(body: &LirBody) -> usize {
    body.facts.0.counted("intervals-edited")
}

/// `intervals` worked out from the body alone, whatever is remembered: what an edited answer is held to.
pub fn intervals_afresh(body: &LirBody) -> IndexMap<u32, Interval> {
    worked_out(body, None, &Frequency::of(body))
}

/// How many answers are remembered: the allocator's rewrites and its trial candidates alternate
/// among a few bodies.
const REMEMBERED: usize = 6;

/// The live interval of every value in this body, weighted.
pub fn intervals(body: &LirBody, index: Option<&Indexes>) -> IndexMap<u32, Interval> {
    intervals_over(body, index, &Frequency::of(body))
}

/// `intervals`, weighted by `busy`, the body's block frequencies, which a caller that asks for several
/// facts of one body finds once.
///
/// An answer is remembered for the next question of the same instructions: a spill is followed by the
/// facts of the body it made, which the spiller's own steps and the class check each asked of it
/// (58% of the asks of compiling `d_faces` were of a body already asked, #559). `index` is
/// `indexed(body)` of this body, as every caller makes it, or none.
pub fn intervals_over(body: &LirBody, index: Option<&Indexes>, busy: &Frequency) -> IndexMap<u32, Interval> {
    let shared = intervals_shared_over(body, index, busy);
    llrm_support::debug::timed("intervals cloned", || (*shared).clone())
}

/// `intervals`, the remembered answer itself: for a caller that only reads it, which would copy every interval of the body to do so.
pub fn intervals_shared(body: &LirBody, index: Option<&Indexes>) -> Arc<IndexMap<u32, Interval>> {
    intervals_shared_over(body, index, &Frequency::of(body))
}

/// `intervals_over` without the copy.
pub fn intervals_shared_over(body: &LirBody, index: Option<&Indexes>, busy: &Frequency) -> Arc<IndexMap<u32, Interval>> {
    if let Some(found) = body.facts.0.stash(|recent: &mut Recent| {
        let at = recent.0.iter().position(|held| held.is_of(body))?;
        // Most recent first.
        let held = recent.0.remove(at);
        let answer = Arc::clone(&held.answer);
        recent.0.insert(0, held);
        Some(answer)
    }) {
        if llrm_support::env_set("LLRM_CHECK_INTERVALS") {
            assert!(*found == worked_out(body, index, busy), "{}: a remembered answer differs from working it out", body.name);
        }
        llrm_support::debug::counted("intervals remembered", true);
        return found;
    }
    llrm_support::debug::counted("intervals remembered", false);
    let numbered;
    let index: &Indexes = match index {
        Some(index) => index,
        None => {
            numbered = indexed_shared(body);
            &numbered
        }
    };
    let shared_index = index_is_numbered(body, index);
    // A body made of the last one's instructions and a few others: its answer is that one's, moved to the new slots, and
    // worked out again only for the values the others name (LLVM's `LiveIntervals` edited across a spill).
    let latest = body.facts.0.stash(|recent: &mut Recent| recent.0.first().cloned());
    let edited = latest.and_then(|held| updated(&held, body, index, busy));
    let (answer, totals) = match edited {
        Some(found) => {
            llrm_support::debug::counted("intervals edited", true);
            body.facts.0.bump("intervals-edited");
            if llrm_support::env_set("LLRM_CHECK_INTERVALS") {
                let whole = worked_out(body, Some(index), busy);
                if found.0 != whole {
                    let mut shown = 0;
                    for (value, expected) in &whole {
                        if found.0.get(value) != Some(expected) && shown < 6 {
                            shown += 1;
                            eprintln!("value {value}:\n edited {:?}\n whole  {:?}", found.0.get(value).map(|one| one.segments.iter().map(|s| (s.start, s.end)).collect::<Vec<_>>()), expected.segments.iter().map(|s| (s.start, s.end)).collect::<Vec<_>>());
                        }
                    }
                    for value in found.0.keys().filter(|value| !whole.contains_key(*value)) {
                        eprintln!("value {value}: edited has it, whole does not");
                    }
                    panic!("{}: an edited answer differs from working it out", body.name);
                }
            }
            found
        }
        None => {
            llrm_support::debug::counted("intervals edited", false);
            llrm_support::debug::timed("intervals worked out", || worked_out_with_totals(body, index, busy, &|_| true))
        }
    };
    let answer = Arc::new(answer);
    let remembered = Arc::new(Remembered::of(body, &answer, &shared_index.unwrap_or_else(|| Arc::new(index.clone())), totals));
    body.facts.0.stash(|recent: &mut Recent| {
        recent.0.insert(0, remembered);
        recent.0.truncate(REMEMBERED);
    });
    answer
}

/// The numbering `indexed_shared` holds for `body`, if `index` is it: kept as it is, not copied.
fn index_is_numbered(body: &LirBody, index: &Indexes) -> Option<Arc<Indexes>> {
    body.facts.0.stash(|held: &mut Numbered| held.0.iter().find(|(_, found)| std::ptr::eq(&**found, index)).map(|(_, found)| Arc::clone(found)))
}

/// The answer for `body` made from `held`'s, where `body` keeps most of the instructions `held` had: those it keeps are
/// at new slots, and the values the others name are worked out again. None where that is not so.
fn updated(held: &Remembered, body: &LirBody, index: &Indexes, busy: &Frequency) -> Option<(IndexMap<u32, Interval>, IndexMap<u32, f64>)> {
    let same_blocks = held.entry == body.entry
        && held.blocks.len() == body.blocks.len()
        && held.blocks.iter().zip(&body.blocks).all(|((at, succ, phis, _), block)| *at == block.at && *succ == block.succ && *phis == block.phis)
        && held.odds == body.odds
        && held.trips == body.loop_trip_counts;
    if !same_blocks {
        return None;
    }
    // A block that is the same instructions needs no looking at: it is where it was, shifted. The others are compared by
    // instruction, for the values the instructions that are not in both name.
    let blocks: Vec<(i64, &Insns)> = held.blocks.iter().map(|(at, _, _, insns)| (*at, insns)).collect();
    let differing = differing_blocks(&blocks, body);
    let mut touched: crate::support::hash::HashSet<u32> = Default::default();
    let changed = changed_runs(&blocks, &held.index, body, index, &differing, &mut |run| {
        for one in run {
            touched.extend(one.defines.iter().chain(&one.uses).copied());
        }
    });
    if changed * 4 > held.count + 16 || touched.len() * 3 > held.answer.len() + 16 {
        return None;
    }
    let shift = Shift::between(&blocks, &held.index, body, index, &differing);
    let (start, end) = (|slot| shift.start(slot), |slot| shift.end(slot));
    let again = llrm_support::debug::timed("intervals among", || worked_among(body, index, busy, &touched));
    let mut answer: IndexMap<u32, Interval> = IndexMap::default();
    let mut totals: IndexMap<u32, f64> = IndexMap::default();
    for (value, kept) in held.answer.iter().filter(|(value, _)| !touched.contains(value)) {
        let segments: Vec<Segment> = kept.segments.iter().map(|segment| Segment { start: start(segment.start), end: end(segment.end) }).collect();
        let mut moved = Interval::new(*value, segments);
        moved.weight = match held.totals.get(value) {
            Some(total) => {
                totals.insert(*value, *total);
                *total / (moved.size() + GRACE) as f64
            }
            None => 0.0,
        };
        answer.insert(*value, moved);
    }
    for (value, total) in held.totals.iter().filter(|(value, _)| !touched.contains(value) && !answer.contains_key(*value)) {
        totals.insert(*value, *total);
    }
    answer.extend(again.0);
    totals.extend(again.1);
    Some((answer, totals))
}

/// `worked_out_with_totals` of the values `only` holds, from where they occur: one pass over the instructions finds each value's
/// occurrences and weights, and the intervals are found from those (`intervals_by_occurrences`) where the walk would take the body's
/// liveness whole and walk every block. For a body with phis, whose arguments are read in other blocks, the walk.
fn worked_among(body: &LirBody, index: &Indexes, busy: &Frequency, only: &crate::support::hash::HashSet<u32>) -> (IndexMap<u32, Interval>, IndexMap<u32, f64>) {
    if body.blocks.iter().any(|block| !block.phis.is_empty()) || llrm_support::env_set("LLRM_WALK_ALL") {
        return worked_out_with_totals(body, index, busy, &|value| only.contains(&value));
    }
    body.facts.0.bump("intervals-by-occurrences");
    crate::analysis::occurrences::Occurrences::scan(body, &|value| only.contains(&value)).intervals(body, index, busy)
}

fn worked_out(body: &LirBody, index: Option<&Indexes>, busy: &Frequency) -> IndexMap<u32, Interval> {
    worked_out_by(body, index, busy, &|_| true)
}

/// The intervals of the values in `only` alone, as `intervals` finds them in `body`: nothing is numbered,
/// walked or weighed for the others. For a caller that adds a few values of its own to a body and asks
/// of those, the others' intervals being the body's, remembered.
pub fn intervals_among(body: &LirBody, index: &Indexes, busy: &Frequency, only: &BTreeSet<u32>) -> IndexMap<u32, Interval> {
    worked_out_by(body, Some(index), busy, &|value| only.contains(&value))
}

/// `intervals_among`, the values named by a test: for values numbered together, a comparison, where a set is
/// a search of a tree for every operand of every instruction of the body.
pub fn intervals_where(body: &LirBody, index: &Indexes, busy: &Frequency, keep: &impl Fn(u32) -> bool) -> IndexMap<u32, Interval> {
    worked_out_by(body, Some(index), busy, keep)
}

fn worked_out_by(body: &LirBody, index: Option<&Indexes>, busy: &Frequency, keep: &impl Fn(u32) -> bool) -> IndexMap<u32, Interval> {
    let owned;
    let index = match index {
        Some(index) => index,
        None => {
            owned = indexed(body);
            &owned
        }
    };
    worked_out_with_totals(body, index, busy, keep).0
}

/// `worked_out_by`, and what each value's references weigh before they are divided by its size.
fn worked_out_with_totals(body: &LirBody, index: &Indexes, busy: &Frequency, keep: &impl Fn(u32) -> bool) -> (IndexMap<u32, Interval>, IndexMap<u32, f64>) {
    body.facts.0.bump("intervals-worked");
    let ranges = _ranges(body, index, keep);
    let totals = llrm_support::debug::timed("intervals weights", || _totals(body, busy, keep));
    let weight = divided(&totals, &ranges);
    let answer = ranges
        .into_iter()
        .map(|(value, one)| {
            let weight = weight.get(&value).copied().unwrap_or(0.0);
            (value, Interval { weight, ..one })
        })
        .collect();
    (answer, totals)
}

/// The `group` run ending at `position`: where it starts.
fn _group_start(block: &LirBlock, position: usize) -> usize {
    let one = &block.insns[position];
    let mut first = position;
    if one.group.is_some() {
        while first > 0 && block.insns[first - 1].group == one.group {
            first -= 1;
        }
    }
    first
}

/// Where each value is live, before anything prices it.
///
/// Python builds `pieces` by iterating sets; only the map's order differs,
/// and nothing reads it in order. (The order is kept as it was all the same.)
fn _ranges(body: &LirBody, index: &Indexes, keep: &impl Fn(u32) -> bool) -> IndexMap<u32, Interval> {
    let found = _walked(body, index, keep);
    if llrm_support::env_set("LLRM_CHECK_RANGES") {
        let reference = _ranges_reference(body, index, keep);
        assert!(found.iter().eq(reference.iter()), "{}: the walk differs from the reference", body.name);
    }
    found
}

/// What is live now in a block's backward walk, in the order values became so: removal leaves a gap, not
/// a shift of all that follows, and a value made live again comes last.
struct Alive {
    order: Vec<Option<(u32, i64)>>,
    at: crate::support::hash::HashMap<u32, usize>,
}

impl Alive {
    fn new() -> Self {
        Self { order: Vec::new(), at: Default::default() }
    }

    /// Nothing alive, the room kept for the next block.
    fn clear(&mut self) {
        self.order.clear();
        self.at.clear();
    }

    fn insert_if_absent(&mut self, value: u32, end: i64) {
        if !self.at.contains_key(&value) {
            self.at.insert(value, self.order.len());
            self.order.push(Some((value, end)));
        }
    }

    fn remove(&mut self, value: u32) -> Option<i64> {
        let at = self.at.remove(&value)?;
        self.order[at].take().map(|(_, end)| end)
    }

    fn contains(&self, value: u32) -> bool {
        self.at.contains_key(&value)
    }

    fn iter(&self) -> impl Iterator<Item = (u32, i64)> + '_ {
        self.order.iter().flatten().copied()
    }
}

pub(crate) fn _walked(body: &LirBody, index: &Indexes, keep: &impl Fn(u32) -> bool) -> IndexMap<u32, Interval> {
    _walked_at(body, index, keep, None)
}

/// The values `keep` says of a body of which only the instructions that name them are given, each block's
/// with the slot each has in the whole body: the body's numbering, blocks and all else are `index`'s.
/// Where an instruction is in a body: its block's number and its own in the block.
pub type Place = (usize, usize);

/// An instruction that names a value: where it is, and whether it defines and whether it reads the value.
pub type Occurrence = (Place, bool, bool);

/// The intervals of `values` (ascending), worked out from where they occur rather than by walking the body: what the walk
/// finds of a value is decided in the blocks it occurs in and those it is live through, and the rest of the body adds
/// nothing. `places[value]` are the instructions that name it, by block then position, once each, and whether each defines
/// and reads it (not what the body's own says, where a caller adds values to it). The answer holds
/// what `intervals_sparse` does, in another order of values; it costs the occurrences and the blocks the values are live
/// in, where the walk costs every value live in every block, hashed.
pub fn intervals_by_occurrences(body: &LirBody, index: &Indexes, values: &[u32], places: &IndexMap<u32, Vec<Occurrence>>) -> IndexMap<u32, Interval> {
    if values.is_empty() {
        return IndexMap::default();
    }
    let count = body.blocks.len();
    let position: crate::support::hash::HashMap<i64, usize> = body.blocks.iter().enumerate().map(|(at, block)| (block.at, at)).collect();
    let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (at, block) in body.blocks.iter().enumerate() {
        for to in &block.succ {
            if let Some(&to) = position.get(to) {
                predecessors[to].push(at);
            }
        }
    }
    let spans: Vec<(i64, i64)> = body.blocks.iter().map(|block| index.span[&block.at]).collect();
    // Marks by block, a mark current when it equals the value's turn: no table is cleared between values.
    let (mut in_mark, mut out_mark, mut written_mark, mut run_mark, mut reach_mark) = (vec![0u32; count], vec![0u32; count], vec![0u32; count], vec![0u32; count], vec![0u32; count]);
    let mut run_of: Vec<usize> = vec![0; count];
    let mut out: IndexMap<u32, Interval> = IndexMap::default();
    let mut turn = 0u32;
    for &value in values {
        let Some(found) = places.get(&value).filter(|found| !found.is_empty()) else { continue };
        turn += 1;
        // By block: the parallel-copy runs it occurs in, as (last position of the run, defined there, read there), ascending.
        let mut by_block: Vec<Vec<(usize, bool, bool)>> = Vec::new();
        let mut work: Vec<usize> = Vec::new();
        // The blocks this value occurs in or is live through: all else is none of its business, so no pass is over every block.
        let mut reached: Vec<usize> = Vec::new();
        for &((block_index, at), defined, used) in found {
            let block = &body.blocks[block_index];
            let end = _group_end(block, at);
            if run_mark[block_index] != turn {
                run_mark[block_index] = turn;
                run_of[block_index] = by_block.len();
                by_block.push(Vec::new());
                if reach_mark[block_index] != turn {
                    reach_mark[block_index] = turn;
                    reached.push(block_index);
                }
            }
            let runs = &mut by_block[run_of[block_index]];
            match runs.last_mut() {
                Some((run, was_defined, was_used)) if *run == end => {
                    *was_defined |= defined;
                    *was_used |= used;
                }
                _ => runs.push((end, defined, used)),
            }
        }
        // Live in a block, and out of it, by the occurrences: read before it is written there, and so up its predecessors until
        // a block writes it. The first run that names it decides: a read, even with a write beside it (the walk takes the
        // group's writes first).
        for &block_index in &reached {
            let runs = &by_block[run_of[block_index]];
            if runs.iter().any(|(_, defined, _)| *defined) {
                written_mark[block_index] = turn;
            }
            if runs.first().is_some_and(|(_, _, used)| *used) {
                in_mark[block_index] = turn;
                work.push(block_index);
            }
        }
        while let Some(block_index) = work.pop() {
            for &before in &predecessors[block_index] {
                if out_mark[before] != turn {
                    out_mark[before] = turn;
                    if reach_mark[before] != turn {
                        reach_mark[before] = turn;
                        reached.push(before);
                    }
                    if written_mark[before] != turn && in_mark[before] != turn {
                        in_mark[before] = turn;
                        work.push(before);
                    }
                }
            }
        }
        reached.sort_unstable();
        let mut segments: Vec<Segment> = Vec::new();
        let mut here: Vec<Segment> = Vec::new();
        for &block_index in &reached {
            let occurs = run_mark[block_index] == turn;
            let live_out = out_mark[block_index] == turn;
            let live_in = in_mark[block_index] == turn;
            if !(occurs || live_out || live_in) {
                continue;
            }
            let block = &body.blocks[block_index];
            let (first, last) = spans[block_index];
            let mut alive: Option<i64> = live_out.then_some(last);
            let mut wrote = false;
            here.clear();
            if occurs {
                for &(end, defined, used) in by_block[run_of[block_index]].iter().rev() {
                    let boundary = index.slot(block, end) + DEF;
                    if defined {
                        wrote = true;
                        here.push(Segment { start: boundary, end: alive.take().unwrap_or(boundary + 1) });
                    }
                    if used && alive.is_none() {
                        alive = Some(boundary);
                    }
                }
            }
            if let Some(end) = alive {
                if end > first {
                    here.push(Segment { start: first, end });
                }
            } else if live_in && !wrote {
                here.push(Segment { start: first, end: last });
            }
            // The block's pieces were found last to first.
            segments.extend(here.iter().rev().copied());
        }
        out.insert(value, Interval::new(value, _merged(segments)));
    }
    out
}

/// The position of the last instruction of the parallel-copy run `position` is in.
pub(crate) fn _group_end(block: &LirBlock, position: usize) -> usize {
    let Some(group) = block.insns[position].group else { return position };
    let mut end = position;
    while end + 1 < block.insns.len() && block.insns[end + 1].group == Some(group) {
        end += 1;
    }
    end
}

pub fn intervals_sparse(sparse: &LirBody, index: &Indexes, starts: &[Vec<i64>], keep: &impl Fn(u32) -> bool) -> IndexMap<u32, Interval> {
    _walked_at(sparse, index, keep, Some(starts))
}

/// `_walked`, each instruction's slot given where `starts` is.
fn _walked_at(body: &LirBody, index: &Indexes, keep: &impl Fn(u32) -> bool, given: Option<&[Vec<i64>]>) -> IndexMap<u32, Interval> {
    let live = llrm_support::debug::timed("intervals liveness", || allocate::live_rows_by(body, keep));
    let _walk = llrm_support::debug::span("intervals walk");
    let mut pieces: IndexMap<u32, Vec<Segment>> = IndexMap::default();
    let mut starts: Vec<i64> = Vec::new();
    let (mut defined, mut used): (Vec<u32>, Vec<u32>) = (Vec::new(), Vec::new());
    // One for every block, emptied at its start: a table grown anew for each block was a rehash of its own.
    let mut alive = Alive::new();
    let mut written: crate::support::hash::HashSet<u32> = Default::default();
    for (block_index, block) in body.blocks.iter().enumerate() {
        let (first, last) = index.span[&block.at];
        // Each instruction's slot: the block's first after its phis' slot, then two for each that is no mark.
        starts.clear();
        if let Some(given) = given {
            starts.extend_from_slice(&given[block_index]);
        } else {
            let mut next = first + PER_INSN;
            for one in &block.insns {
                starts.push(next);
                if !one.is_meta() {
                    next += PER_INSN;
                }
            }
        }
        alive.clear();
        for one in live.leaving(block.at).filter(|one| keep(*one)) {
            alive.insert_if_absent(one, last);
        }
        written.clear();
        let mut position = block.insns.len() as i64 - 1;
        while position >= 0 {
            let at = position as usize;
            let first_in_group = _group_start(block, at);
            let group = &block.insns[first_in_group..=at];
            let boundary = starts[at] + DEF;
            defined.clear();
            for item in group {
                for value in &item.defines {
                    if keep(*value) && !defined.contains(value) {
                        defined.push(*value);
                    }
                }
            }
            for &value in &defined {
                written.insert(value);
                let end = alive.remove(value).unwrap_or(boundary + 1);
                pieces.entry(value).or_default().push(Segment { start: boundary, end });
            }
            used.clear();
            for item in group {
                for value in &item.uses {
                    if keep(*value) && !used.contains(value) {
                        used.push(*value);
                    }
                }
            }
            for &value in &used {
                alive.insert_if_absent(value, boundary);
            }
            position = first_in_group as i64 - 1;
        }
        // A phi's result is defined at the top of the block.
        for phi in block.phis.iter().filter(|phi| keep(phi.result)) {
            written.insert(phi.result);
            let end = alive.remove(phi.result).unwrap_or(first + DEF + 1);
            pieces.entry(phi.result).or_default().push(Segment { start: first + DEF, end });
        }
        // Whatever is still alive arrived from a predecessor.
        for (value, end) in alive.iter() {
            if end > first {
                pieces.entry(value).or_default().push(Segment { start: first, end });
            }
        }
        // Live through: in at the top, out at the bottom, untouched between.
        for value in live.entering(block.at) {
            if !written.contains(&value) && !alive.contains(value) {
                pieces.entry(value).or_default().push(Segment { start: first, end: last });
            }
        }
    }
    drop(_walk);
    llrm_support::debug::timed("intervals merge", || {
        pieces.into_iter().map(|(value, runs)| (value, Interval::new(value, _merged(runs)))).collect()
    })
}

/// `_ranges` as it was written, over a hashed map of slots, a map that shifts on removal and a set per group,
/// which `LLRM_CHECK_RANGES=1` and the tests hold the walk below to.
///
/// Python builds `pieces` by iterating sets; only the map's order differs,
/// and nothing reads it in order.
pub(crate) fn _ranges_reference(body: &LirBody, index: &Indexes, keep: &impl Fn(u32) -> bool) -> IndexMap<u32, Interval> {
    let live = allocate::live_rows_by(body, keep);
    let mut pieces: IndexMap<u32, Vec<Segment>> = IndexMap::default();
    for block in &body.blocks {
        let (first, last) = index.span[&block.at];
        let mut alive: IndexMap<u32, i64> = live.leaving(block.at).filter(|one| keep(*one)).map(|one| (one, last)).collect();
        let mut written: BTreeSet<u32> = BTreeSet::new();
        let mut position = block.insns.len() as i64 - 1;
        while position >= 0 {
            let at = position as usize;
            let one = &block.insns[at];
            let first_in_group = _group_start(block, at);
            let group = &block.insns[first_in_group..=at];
            let slot = if one.group.is_some() {
                index.at[&key(group.last().expect("a group holds its last"))]
            } else {
                index.at[&key(one)]
            };
            let boundary = slot + DEF;
            let defined: IndexSet<u32> = group.iter().flat_map(|item| item.defines.iter().copied()).filter(|value| keep(*value)).collect();
            for value in defined {
                written.insert(value);
                let end = alive.shift_remove(&value).unwrap_or(boundary + 1);
                pieces.entry(value).or_default().push(Segment { start: boundary, end });
            }
            let used: IndexSet<u32> = group.iter().flat_map(|item| item.uses.iter().copied()).filter(|value| keep(*value)).collect();
            for value in used {
                alive.entry(value).or_insert(boundary);
            }
            position = first_in_group as i64 - 1;
        }
        // A phi's result is defined at the top of the block.
        for phi in block.phis.iter().filter(|phi| keep(phi.result)) {
            written.insert(phi.result);
            let end = alive.shift_remove(&phi.result).unwrap_or(first + DEF + 1);
            pieces.entry(phi.result).or_default().push(Segment { start: first + DEF, end });
        }
        // Whatever is still alive arrived from a predecessor.
        for (value, end) in &alive {
            if *end > first {
                pieces.entry(*value).or_default().push(Segment { start: first, end: *end });
            }
        }
        // Live through: in at the top, out at the bottom, untouched between.
        for value in live.entering(block.at) {
            if !written.contains(&value) && !alive.contains_key(&value) {
                pieces.entry(value).or_default().push(Segment { start: first, end: last });
            }
        }
    }
    pieces.into_iter().map(|(value, runs)| (value, Interval::new(value, _merged(runs)))).collect()
}

/// Join overlapping segments, retaining touching definition boundaries.
pub fn _merged(mut runs: Vec<Segment>) -> Vec<Segment> {
    runs.sort_by_key(|x| (x.start, x.end));
    let mut out: Vec<Segment> = Vec::new();
    for one in runs {
        if let Some(last) = out.last_mut() {
            if one.start < last.end {
                *last = Segment { start: last.start, end: last.end.max(one.end) };
                continue;
            }
        }
        out.push(one);
    }
    out
}


/// How deeply each block is nested in loops.
pub fn depths(body: &LirBody) -> IndexMap<i64, u32> {
    depths_in(body, &loopy::loops(&body.blocks, Some(body.entry)))
}

/// `depths`, from loops already found.
pub fn depths_in(body: &LirBody, loops: &[loopy::Loop]) -> IndexMap<i64, u32> {
    let mut out: IndexMap<i64, u32> = body.blocks.iter().map(|block| (block.at, 0)).collect();
    for found in loops {
        for at in &found.body {
            if let Some(depth) = out.get_mut(at) {
                *depth += 1;
            }
        }
    }
    out
}

/// What one reference costs per level of loop nesting: the weight region
/// splitting and slot sharing still give a block (`spillplacement`,
/// `splitkit`, `spiller`), which block frequencies made larger code in
/// their hands (#191).
pub const PER_LEVEL: i64 = 10;

/// `float(PER_LEVEL ** depth)`.
pub fn level(depth: u32) -> f64 {
    PER_LEVEL.pow(depth) as f64
}

/// Added to the size before dividing. LLVM's `25 * InstrDist`.
pub const GRACE: i64 = 25 * PER_INSN;

/// `references weighted by block frequency / (live slots + grace)`.
#[allow(dead_code)]
fn _weights(body: &LirBody, busy: &Frequency, ranges: &IndexMap<u32, Interval>, keep: &impl Fn(u32) -> bool) -> IndexMap<u32, f64> {
    divided(&_totals(body, busy, keep), ranges)
}

/// `references weighted by block frequency`, before the division.
fn _totals(body: &LirBody, busy: &Frequency, keep: &impl Fn(u32) -> bool) -> IndexMap<u32, f64> {
    let mut total: IndexMap<u32, f64> = IndexMap::default();
    for block in &body.blocks {
        let each = busy.block(block.at);
        for one in &block.insns {
            for value in one.defines.iter().chain(&one.uses).filter(|value| keep(**value)) {
                *total.entry(*value).or_insert(0.0) += each;
            }
        }
    }
    total
}

pub(crate) fn divided(total: &IndexMap<u32, f64>, ranges: &IndexMap<u32, Interval>) -> IndexMap<u32, f64> {
    total
        .iter()
        .map(|(value, found)| {
            let size = match ranges.get(value) {
                Some(one) => one.size() + GRACE,
                None => GRACE,
            };
            (*value, *found / size as f64)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    /// The numbering of slots is this file's alone: everything else asks for a point (`def_point`, `window_end`, `slot`,
    /// `spill_size`) and never does arithmetic on the constants, so the numbering can change while what is asked stays
    /// true. Test code is free to name them.
    #[test]
    fn test_nothing_outside_the_numbering_does_arithmetic_on_slots() {
        let mut found = Vec::new();
        let mut dirs = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(dir).expect("a source directory") {
                let path = entry.expect("an entry").path();
                if path.is_dir() {
                    dirs.push(path);
                    continue;
                }
                let name = path.to_string_lossy().into_owned();
                if !name.ends_with(".rs") || name.ends_with("analysis/intervals.rs") || name.ends_with("_tests.rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).expect("a source file");
                let code = text.split("#[cfg(test)]").next().unwrap_or_default();
                for token in ["PER_INSN", "intervals::DEF", "ranges::DEF", "GRACE"] {
                    if code.contains(token) {
                        found.push(format!("{name}: {token}"));
                    }
                }
            }
        }
        assert!(found.is_empty(), "slot arithmetic outside the numbering: {found:?}");
    }
}
