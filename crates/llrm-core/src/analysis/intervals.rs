//! Port of `qbopt/analysis/intervals.py`: where each value is live, as
//! ranges rather than as sets per block.
//!
//! LLVM's `LiveIntervals`, two slots per instruction: where it reads, and
//! where it writes.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::support::hash::{IndexMap, IndexSet};

use crate::analysis::loops as loopy;
use crate::backend::allocate;
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::model::mir::MirBlock;

// Points per instruction: where it reads, and where it writes.
pub const USE: i64 = 0;
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

impl Indexes {
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

/// The live interval of every value in this body, weighted.
pub fn intervals(body: &LirBody, index: Option<&Indexes>) -> IndexMap<u32, Interval> {
    let owned;
    let index = match index {
        Some(index) => index,
        None => {
            owned = indexed(body);
            &owned
        }
    };
    let ranges = _ranges(body, index);
    let weight = _weights(body, index, &ranges);
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
/// and nothing reads it in order.
fn _ranges(body: &LirBody, index: &Indexes) -> IndexMap<u32, Interval> {
    let (live_in, live_out) = allocate::live(body);
    let mut pieces: IndexMap<u32, Vec<Segment>> = IndexMap::default();
    for block in &body.blocks {
        let (first, last) = index.span[&block.at];
        let mut alive: IndexMap<u32, i64> = live_out[&block.at].iter().map(|one| (*one, last)).collect();
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
            let defined: IndexSet<u32> = group.iter().flat_map(|item| item.defines.iter().copied()).collect();
            for value in defined {
                written.insert(value);
                let end = alive.shift_remove(&value).unwrap_or(boundary + 1);
                pieces.entry(value).or_default().push(Segment { start: boundary, end });
            }
            let used: IndexSet<u32> = group.iter().flat_map(|item| item.uses.iter().copied()).collect();
            for value in used {
                alive.entry(value).or_insert(boundary);
            }
            position = first_in_group as i64 - 1;
        }
        // A phi's result is defined at the top of the block.
        for phi in &block.phis {
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
        for value in &live_in[&block.at] {
            if !written.contains(value) && !alive.contains_key(value) {
                pieces.entry(*value).or_default().push(Segment { start: first, end: last });
            }
        }
    }
    pieces
        .into_iter()
        .map(|(value, runs)| (value, Interval::new(value, _merged(runs))))
        .collect()
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

/// `loopy.loops` reads only `at` and `succ`; MIR blocks carry both.
pub fn _graph(blocks: &[LirBlock]) -> Vec<MirBlock> {
    blocks
        .iter()
        .map(|block| MirBlock::new(block.at, Vec::new(), Vec::new(), block.succ.clone()))
        .collect()
}

/// How deeply each block is nested in loops.
pub fn depths(body: &LirBody) -> IndexMap<i64, u32> {
    let mut out: IndexMap<i64, u32> = body.blocks.iter().map(|block| (block.at, 0)).collect();
    for found in loopy::loops(&_graph(&body.blocks), Some(body.entry)) {
        for at in &found.body {
            if let Some(depth) = out.get_mut(at) {
                *depth += 1;
            }
        }
    }
    out
}

/// What one reference costs per level of loop nesting.
pub const PER_LEVEL: i64 = 10;

/// Added to the size before dividing. LLVM's `25 * InstrDist`.
pub const GRACE: i64 = 25 * PER_INSN;

/// `float(PER_LEVEL ** depth)`.
pub fn level(depth: u32) -> f64 {
    PER_LEVEL.pow(depth) as f64
}

/// What spilling each value would cost. See `_weights` for the formula.
pub fn weights(body: &LirBody, index: Option<&Indexes>) -> IndexMap<u32, f64> {
    let owned;
    let index = match index {
        Some(index) => index,
        None => {
            owned = indexed(body);
            &owned
        }
    };
    _weights(body, index, &_ranges(body, index))
}

/// `references weighted by loop depth / (live slots + grace)`.
fn _weights(body: &LirBody, _index: &Indexes, ranges: &IndexMap<u32, Interval>) -> IndexMap<u32, f64> {
    let deep = depths(body);
    let mut total: IndexMap<u32, f64> = IndexMap::default();
    for block in &body.blocks {
        let each = level(deep.get(&block.at).copied().unwrap_or(0));
        for one in &block.insns {
            for value in one.defines.iter().chain(&one.uses) {
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
