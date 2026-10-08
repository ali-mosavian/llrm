//! Port of `qbopt/analysis/intervals.py`: where each value is live, as
//! ranges rather than as sets per block.
//!
//! LLVM's `LiveIntervals`, two slots per instruction: where it reads, and
//! where it writes.

use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

use crate::analysis::frequency::Frequency;
use crate::support::hash::{IndexMap, IndexSet};

use crate::analysis::loops as loopy;
use crate::backend::allocate;
use crate::model::lir::{Insn, LirBlock, LirBody};

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
    let mut at = IndexMap::default();
    let mut span = IndexMap::default();
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

thread_local! {
    static NUMBERED: std::cell::RefCell<Option<(Vec<Arc<Insn>>, Vec<(i64, usize)>, Rc<Indexes>)>> = const { std::cell::RefCell::new(None) };
    static INDEXED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has numbered a body for `indexed_shared`, for a test that asking again of one body
/// does not.
pub fn numbered() -> usize {
    INDEXED.with(std::cell::Cell::get)
}

/// `indexed(body)`, remembered for the next ask of the same instructions (by identity, in the same blocks).
pub fn indexed_shared(body: &LirBody) -> Rc<Indexes> {
    NUMBERED.with(|held| {
        let mut held = held.borrow_mut();
        if let Some((insns, blocks, found)) = held.as_ref() {
            let same = blocks.len() == body.blocks.len()
                && blocks.iter().zip(&body.blocks).all(|((at, count), block)| *at == block.at && *count == block.insns.len())
                && insns.iter().zip(body.blocks.iter().flat_map(|block| block.insns.iter())).all(|(held, one)| Arc::ptr_eq(held, one));
            if same {
                return Rc::clone(found);
            }
        }
        INDEXED.with(|count| count.set(count.get() + 1));
        let found = Rc::new(indexed(body));
        *held = Some((body.blocks.iter().flat_map(|block| block.insns.iter().cloned()).collect(), body.blocks.iter().map(|block| (block.at, block.insns.len())).collect(), Rc::clone(&found)));
        found
    })
}

/// What an answer of `intervals_over` was made of: the body's instructions by identity (held, so
/// that an address is not reused while it is remembered), its blocks and phis, and what its
/// frequencies read.
struct Remembered {
    entry: i64,
    blocks: Vec<(i64, Vec<i64>, Vec<crate::model::lir::Phi>, usize)>,
    insns: Vec<Arc<Insn>>,
    odds: crate::model::lir::BlockOdds,
    trips: Vec<(i64, i64)>,
    answer: Rc<IndexMap<u32, Interval>>,
}

impl Remembered {
    fn of(body: &LirBody, answer: &IndexMap<u32, Interval>) -> Self {
        Self {
            entry: body.entry,
            blocks: body.blocks.iter().map(|block| (block.at, block.succ.clone(), block.phis.clone(), block.insns.len())).collect(),
            insns: body.blocks.iter().flat_map(|block| block.insns.iter().cloned()).collect(),
            odds: body.odds.clone(),
            trips: body.loop_trip_counts.clone(),
            answer: Rc::new(answer.clone()),
        }
    }

    fn is_of(&self, body: &LirBody) -> bool {
        self.entry == body.entry
            && self.blocks.len() == body.blocks.len()
            && self.blocks.iter().zip(&body.blocks).all(|((at, succ, phis, count), block)| *at == block.at && *count == block.insns.len() && *succ == block.succ && *phis == block.phis)
            && self.insns.iter().zip(body.blocks.iter().flat_map(|block| block.insns.iter())).all(|(held, one)| Arc::ptr_eq(held, one))
            && self.odds == body.odds
            && self.trips == body.loop_trip_counts
    }
}

thread_local! {
    static RECENT: std::cell::RefCell<Vec<Remembered>> = const { std::cell::RefCell::new(Vec::new()) };
    static WORKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has worked out intervals, for a test that asking again of one body does not.
pub fn worked() -> usize {
    WORKED.with(std::cell::Cell::get)
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
    if let Some(found) = RECENT.with(|recent| {
        let mut recent = recent.borrow_mut();
        let at = recent.iter().position(|held| held.is_of(body))?;
        // Most recent first.
        let held = recent.remove(at);
        let answer = Rc::clone(&held.answer);
        recent.insert(0, held);
        Some(answer)
    }) {
        if std::env::var_os("LLRM_CHECK_INTERVALS").is_some() {
            assert!(*found == worked_out(body, index, busy), "{}: a remembered answer differs from working it out", body.name);
        }
        llrm_support::debug::counted("intervals remembered", true);
        return llrm_support::debug::timed("intervals cloned", || (*found).clone());
    }
    llrm_support::debug::counted("intervals remembered", false);
    let answer = llrm_support::debug::timed("intervals worked out", || worked_out(body, index, busy));
    RECENT.with(|recent| {
        let mut recent = recent.borrow_mut();
        recent.insert(0, Remembered::of(body, &answer));
        recent.truncate(REMEMBERED);
    });
    answer
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
    WORKED.with(|worked| worked.set(worked.get() + 1));
    let owned;
    let index = match index {
        Some(index) => index,
        None => {
            owned = indexed(body);
            &owned
        }
    };
    let ranges = _ranges(body, index, keep);
    let weight = llrm_support::debug::timed("intervals weights", || _weights(body, busy, &ranges, keep));
    ranges
        .into_iter()
        .map(|(value, one)| {
            let weight = weight.get(&value).copied().unwrap_or(0.0);
            (value, Interval { weight, ..one })
        })
        .collect()
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
    if std::env::var_os("LLRM_CHECK_RANGES").is_some() {
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
fn _weights(body: &LirBody, busy: &Frequency, ranges: &IndexMap<u32, Interval>, keep: &impl Fn(u32) -> bool) -> IndexMap<u32, f64> {
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
        .into_iter()
        .map(|(value, found)| {
            let size = match ranges.get(&value) {
                Some(one) => one.size() + GRACE,
                None => GRACE,
            };
            (value, found / size as f64)
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
