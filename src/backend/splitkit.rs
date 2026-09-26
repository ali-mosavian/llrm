//! Port of `qbopt/backend/splitkit.py`: splitting a live range instead of
//! spilling the whole of it.
//!
//! LLVM's `SplitKit`: `placed`, `local` and `per_block` choose where a piece
//! holds the value (`tryRegionSplit`, `tryLocalSplit`, `tryBlockSplit`), and
//! `carved` makes it (`SplitEditor`).

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use iced_x86::Register;
use crate::support::hash::IndexMap;

use crate::analysis::intervals::{self as ranges, Indexes, Interval, Segment};
use crate::analysis::loops;
use crate::backend::spillplacement::{self, Border, Constraint};
use crate::backend::{allocate, spiller};
use crate::model::ir::{self, Held, Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody};

/// Carve spilled address operands -- base, index, selector -- into the
/// natural loops that reuse them.
pub fn loop_bases(body: &LirBody, values: &BTreeSet<u32>) -> (LirBody, BTreeSet<u32>) {
    if values.is_empty() {
        return (body.clone(), BTreeSet::new());
    }
    let mut result = body.clone();
    let mut kept: BTreeSet<u32> = BTreeSet::new();
    for found in loops::loops(&ranges::_graph(&result.blocks), Some(result.entry)) {
        let inside = &found.body;
        let references: IndexMap<u32, IndexMap<i64, Vec<usize>>> =
            values.iter().map(|value| (*value, _references(&result, *value))).collect();
        let candidates: BTreeSet<u32> = references
            .iter()
            .filter(|(value, found)| {
                found.keys().any(|at| inside.contains(at))
                    && found.keys().any(|at| !inside.contains(at))
                    && result
                        .blocks
                        .iter()
                        .filter(|block| inside.contains(&block.at))
                        .flat_map(|block| &block.insns)
                        .filter_map(|one| one.what.as_ref())
                        .flat_map(|what| what.dests.iter().chain(&what.sources))
                        .any(|place| {
                            matches!(place, Loc::Mem(cell) if [cell.base, cell.index, cell.selector]
                                .iter()
                                .flatten()
                                .any(|operand| operand.value == **value))
                        })
            })
            .map(|(value, _found)| *value)
            .collect();
        for value in candidates {
            let widths = _widths(&result);
            let Some(width) = widths.get(&value).copied() else {
                continue;
            };
            let fresh = _next_value(&result);
            let Some(carved) = carved(&result, value, fresh, width, &Region::blocks(&result, inside.iter().copied()))
            else {
                continue;
            };
            result = carved;
            kept.insert(fresh);
        }
    }
    (result, kept)
}

/// Per block, the positions in it that name this value.
fn _references(body: &LirBody, value: u32) -> IndexMap<i64, Vec<usize>> {
    let mut out = IndexMap::default();
    for block in &body.blocks {
        let found: Vec<usize> = block
            .insns
            .iter()
            .enumerate()
            .filter(|(_position, one)| one.defines.contains(&value) || one.uses.contains(&value))
            .map(|(position, _one)| position)
            .collect();
        if !found.is_empty() {
            out.insert(block.at, found);
        }
    }
    out
}

/// Where a piece lives: per block, the instruction ranges `[from, to)` it
/// covers, sorted and disjoint. A range from 0 takes the block's entry, one
/// to its length its exit.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Region {
    pub spans: BTreeMap<i64, Vec<(usize, usize)>>,
}

impl Region {
    /// These blocks, whole.
    pub fn blocks(body: &LirBody, blocks: impl IntoIterator<Item = i64>) -> Self {
        let length: IndexMap<i64, usize> = body.blocks.iter().map(|block| (block.at, block.insns.len())).collect();
        let mut out = Self::default();
        for at in blocks {
            out.add(at, 0, length[&at]);
        }
        out
    }

    pub fn add(&mut self, block: i64, from: usize, to: usize) {
        let ranges = self.spans.entry(block).or_default();
        ranges.push((from, to));
        ranges.sort_unstable();
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (from, to) in ranges.drain(..) {
            match merged.last_mut() {
                Some(last) if from <= last.1 => last.1 = last.1.max(to),
                _ => merged.push((from, to)),
            }
        }
        *ranges = merged;
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// Whether the piece holds the value as control enters `block`.
    pub fn enters(&self, block: i64) -> bool {
        self.spans.get(&block).is_some_and(|ranges| ranges.first().is_some_and(|one| one.0 == 0))
    }

    /// Whether the piece holds the value as control leaves `block`.
    pub fn leaves(&self, block: &LirBlock) -> bool {
        self.spans.get(&block.at).is_some_and(|ranges| ranges.last().is_some_and(|one| one.1 == block.insns.len()))
    }

    fn covers(&self, block: i64, position: usize) -> bool {
        self.spans.get(&block).is_some_and(|ranges| ranges.iter().any(|(from, to)| *from <= position && position < *to))
    }

    /// The slots the piece spans, as `intervals::indexed` numbers them.
    pub fn segments(&self, body: &LirBody, index: &Indexes) -> Vec<Segment> {
        let length: IndexMap<i64, usize> = body.blocks.iter().map(|block| (block.at, block.insns.len())).collect();
        let mut out = Vec::new();
        for (at, ranges) in &self.spans {
            let (first, last) = index.span[at];
            let slot = |position: usize| first + ranges::PER_INSN * (position as i64 + 1);
            for (from, to) in ranges {
                let start = if *from == 0 { first } else { slot(*from) };
                let end = if *to == length[at] { last } else { slot(*to) };
                out.push(Segment { start, end });
            }
        }
        out
    }

    /// This region in a body `carved_moving` changed.
    pub fn moved(&self, moved: &Moved) -> Self {
        let mut out = Self::default();
        for (at, ranges) in &self.spans {
            for (from, to) in ranges {
                match moved.get(at) {
                    Some(shift) => out.add(*at, shift[*from], shift[*to]),
                    None => out.add(*at, *from, *to),
                }
            }
        }
        out
    }

    /// Ranges that start where control only enters from outside begin at
    /// their first reference; ranges that end where it only leaves stop
    /// after their last. The copies are the same; the piece is shorter.
    pub fn trimmed(&self, body: &LirBody, value: u32) -> Self {
        let mut predecessors: IndexMap<i64, Vec<i64>> = IndexMap::default();
        for block in &body.blocks {
            for next in &block.succ {
                predecessors.entry(*next).or_default().push(block.at);
            }
        }
        let mut out = Self::default();
        for block in &body.blocks {
            let Some(ranges) = self.spans.get(&block.at) else { continue };
            let named: Vec<usize> =
                (0..block.insns.len()).filter(|at| block.insns[*at].defines.contains(&value) || block.insns[*at].uses.contains(&value)).collect();
            let outside_in = block.at != body.entry
                && predecessors.get(&block.at).is_none_or(|all| all.iter().all(|one| !self.leaves(&body.blocks[body.blocks.iter().position(|b| b.at == *one).expect("a block")])));
            let outside_out = block.succ.iter().all(|next| !self.enters(*next));
            for (index, (from, to)) in ranges.iter().copied().enumerate() {
                let inner: Vec<usize> = named.iter().copied().filter(|at| from <= *at && *at < to).collect();
                let from = if index == 0 && from == 0 && outside_in { inner.first().copied().unwrap_or(to) } else { from };
                let to = if index == ranges.len() - 1 && to == block.insns.len() && outside_out { inner.last().map_or(from, |last| last + 1) } else { to };
                if from < to {
                    out.add(block.at, from, to);
                }
            }
        }
        out
    }

    /// Ranges widened to whole parallel groups: a copy never lands inside one.
    fn snapped(&self, body: &LirBody) -> Self {
        let mut out = Self::default();
        for block in &body.blocks {
            let Some(ranges) = self.spans.get(&block.at) else { continue };
            let inside = |position: usize| {
                0 < position
                    && position < block.insns.len()
                    && block.insns[position].group.is_some()
                    && block.insns[position].group == block.insns[position - 1].group
            };
            for (mut from, mut to) in ranges.iter().copied() {
                while inside(from) {
                    from -= 1;
                }
                while inside(to) {
                    to += 1;
                }
                out.add(block.at, from, to);
            }
        }
        out
    }
}

/// Where a copy between a value and its piece goes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Crossing {
    /// Before instruction `position` of `block`, or at its end.
    Inside { block: i64, position: usize },
    /// On the edge `from -> to`.
    Edge { from: i64, to: i64 },
}

/// Every point the value crosses the piece's border while live, and whether
/// it enters (a copy into the piece) or leaves (a copy back). A piece that
/// never writes the value still equals it, so leaving needs no copy.
/// `carved` places these copies and `_benefit` prices them: one fact.
pub fn crossings(body: &LirBody, value: u32, region: &Region, live_in: &allocate::Live, live_out: &allocate::Live) -> Vec<(Crossing, bool)> {
    let written = body.blocks.iter().any(|block| {
        region.spans.get(&block.at).is_some_and(|ranges| {
            ranges.iter().any(|(from, to)| block.insns[*from..*to].iter().any(|one| one.defines.contains(&value)))
        })
    });
    let mut out = Vec::new();
    for block in &body.blocks {
        if let Some(ranges) = region.spans.get(&block.at) {
            let before = _live_before(block, value, live_out[&block.at].contains(&value));
            for (from, to) in ranges {
                if *from > 0 && before[*from] {
                    out.push((Crossing::Inside { block: block.at, position: *from }, true));
                }
                if written && *to < block.insns.len() && before[*to] {
                    out.push((Crossing::Inside { block: block.at, position: *to }, false));
                }
            }
        }
        let leaving = region.leaves(block);
        for next in &block.succ {
            let entering = region.enters(*next);
            let across = live_in.get(next).is_some_and(|live| live.contains(&value))
                || body.blocks.iter().filter(|one| one.at == *next).flat_map(|one| &one.phis).any(|phi| phi.incoming.contains(&(block.at, value)));
            if leaving != entering && across && (entering || written) {
                out.push((Crossing::Edge { from: block.at, to: *next }, entering));
            }
        }
    }
    let entry = body.blocks.iter().find(|block| block.at == body.entry);
    if region.enters(body.entry) && entry.is_some_and(|block| live_in[&block.at].contains(&value) && !block.arrives().contains(&value)) {
        out.push((Crossing::Inside { block: body.entry, position: 0 }, true));
    }
    out
}

/// Whether `value` is live just before each position of `block`, and at its end.
fn _live_before(block: &LirBlock, value: u32, out: bool) -> Vec<bool> {
    let mut before = vec![out; block.insns.len() + 1];
    let mut live = out;
    let mut index = block.insns.len();
    while index > 0 {
        let first = allocate::_group_start(block, index - 1);
        let group = &block.insns[first..index];
        if group.iter().any(|one| one.uses.contains(&value)) {
            live = true;
        } else if group.iter().any(|one| one.defines.contains(&value)) {
            live = false;
        }
        for position in first..index {
            before[position] = live;
        }
        index = first;
    }
    before
}

/// `body` with `region` reading a fresh value, joined by copies where
/// `crossings` puts them, or None where nothing changes. An edge copy goes
/// at the end of a source with one successor, else at the start of a target
/// every predecessor of which needs the same copy, else in a new block on
/// the edge.
pub fn carved(body: &LirBody, value: u32, fresh: u32, width: u32, region: &Region) -> Option<LirBody> {
    carved_moving(body, value, fresh, width, region).map(|(cut, _moved)| cut)
}

/// Where each block's positions went: old position (and its length) -> new.
pub type Moved = IndexMap<i64, Vec<usize>>;

/// `carved`, and where it moved every instruction, for regions planned
/// on `body` still to be carved.
pub fn carved_moving(body: &LirBody, value: u32, fresh: u32, width: u32, region: &Region) -> Option<(LirBody, Moved)> {
    let region = region.snapped(body).trimmed(body, value);
    let referenced = body.blocks.iter().any(|block| {
        region.spans.get(&block.at).is_some_and(|ranges| {
            ranges.iter().any(|(from, to)| block.insns[*from..*to].iter().any(|one| one.defines.contains(&value) || one.uses.contains(&value)))
        })
    });
    if !referenced {
        return None;
    }
    let (live_in, live_out) = allocate::live(body);
    let found = crossings(body, value, &region, &live_in, &live_out);
    let mut predecessors: IndexMap<i64, Vec<i64>> = IndexMap::default();
    for block in &body.blocks {
        for next in &block.succ {
            predecessors.entry(*next).or_default().push(block.at);
        }
    }
    let at_of: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    // (block, position) -> copies before it; usize::MAX is before the terminator.
    let mut inserted: BTreeMap<(i64, usize), Vec<bool>> = BTreeMap::new();
    let mut bridges: Vec<(i64, i64, bool)> = Vec::new();
    let edges: Vec<(i64, i64, bool)> =
        found.iter().filter_map(|(at, entering)| if let Crossing::Edge { from, to } = at { Some((*from, *to, *entering)) } else { None }).collect();
    for (at, entering) in &found {
        if let Crossing::Inside { block, position } = at {
            inserted.entry((*block, *position)).or_default().push(*entering);
        }
    }
    for (from, to, entering) in &edges {
        let same = predecessors[to].iter().all(|one| edges.contains(&(*one, *to, *entering)));
        let every = at_of[from].succ.iter().all(|next| edges.contains(&(*from, *next, *entering)));
        if every && !at_of[from].insns.is_empty() {
            if !inserted.get(&(*from, usize::MAX)).is_some_and(|had| had.contains(entering)) {
                inserted.entry((*from, usize::MAX)).or_default().push(*entering);
            }
        } else if at_of[from].succ.len() == 1 && !at_of[from].insns.is_empty() {
            inserted.entry((*from, usize::MAX)).or_default().push(*entering);
        } else if same && *to != body.entry && !at_of[to].arrives().contains(&value) {
            if !inserted.get(&(*to, 0)).is_some_and(|had| had.contains(entering)) {
                inserted.entry((*to, 0)).or_default().push(*entering);
            }
        } else if at_of[from].insns.is_empty() {
            return None;
        } else {
            bridges.push((*from, *to, *entering));
        }
    }
    // `defFromParent`: a value that reads nothing is remade, not copied.
    let remade = spiller::recomputed(body, value);
    let copy = |beside: &Insn, entering: bool| {
        let into = if entering { fresh } else { value };
        match &remade {
            Some(define) => {
                let at = beside.covers.map_or(beside.at, |covers| covers.0);
                let mut made = (*spiller::_renamed(define, &IndexMap::from_iter([(value, into)]))).clone();
                made.at = beside.at;
                made.covers = Some((at, at));
                Arc::new(made)
            }
            None if entering => _copy(beside, fresh, value, width),
            None => _copy(beside, value, fresh, width),
        }
    };
    let rename = IndexMap::from_iter([(value, fresh)]);
    let mut blocks: Vec<LirBlock> = Vec::new();
    let mut moved: Moved = IndexMap::default();
    for block in &body.blocks {
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        let mut shift: Vec<usize> = Vec::with_capacity(block.insns.len() + 1);
        let end = if block.insns.last().is_some_and(|one| _terminates(one)) { block.insns.len() - 1 } else { block.insns.len() };
        for position in 0..=block.insns.len() {
            let beside = block.insns.get(position).or(block.insns.last());
            if let (Some(beside), Some(copies)) = (beside, inserted.get(&(block.at, position))) {
                insns.extend(copies.iter().map(|entering| copy(beside, *entering)));
            }
            if position == end {
                if let (Some(beside), Some(copies)) = (beside, inserted.get(&(block.at, usize::MAX))) {
                    insns.extend(copies.iter().map(|entering| copy(beside, *entering)));
                }
            }
            shift.push(insns.len());
            if let Some(one) = block.insns.get(position) {
                insns.push(if region.covers(block.at, position) { _renamed(one, &rename) } else { Arc::clone(one) });
            }
        }
        moved.insert(block.at, shift);
        blocks.push(block.with_insns(insns));
    }
    let mut by_at: IndexMap<i64, LirBlock> = blocks.iter().map(|block| (block.at, block.clone())).collect();
    let mut made: Vec<LirBlock> = Vec::new();
    let mut next_at = body.blocks.iter().map(|block| block.at).max().unwrap_or(0) + 1;
    for (source, outside, entering) in bridges {
        let bridge = next_at;
        next_at += 1;
        let original = by_at[&source].clone();
        let beside = Arc::clone(original.insns.last().expect("checked above"));
        let rewritten: Vec<Arc<Insn>> = original
            .insns
            .iter()
            .map(|one| match &one.what {
                Some(what) if what.target == Some(outside) => {
                    let mut made = (**one).clone();
                    made.what = Some(Semantics { target: Some(bridge), ..what.clone() });
                    Arc::new(made)
                }
                _ => Arc::clone(one),
            })
            .collect();
        by_at.insert(
            source,
            LirBlock { succ: original.succ.iter().map(|one| if *one == outside { bridge } else { *one }).collect(), ..original.with_insns(rewritten) },
        );
        let mut jump = Insn::new(
            beside.at,
            Some((beside.at, beside.at)),
            Some(Semantics { name: Some("jmp".to_owned()), target: Some(outside), ..Semantics::new(Operation::Jump) }),
            Vec::new(),
            Vec::new(),
        );
        jump.op = beside.op.clone();
        made.push(LirBlock { succ: vec![outside], ..LirBlock::new(bridge, vec![copy(&beside, entering), Arc::new(jump)]) });
        // A phi in the target now arrives from the bridge.
        if let Some(target) = by_at.get_mut(&outside) {
            for phi in &mut target.phis {
                for (from, _) in &mut phi.incoming {
                    if *from == source {
                        *from = bridge;
                    }
                }
            }
        }
    }
    let mut out: Vec<LirBlock> = body.blocks.iter().map(|block| by_at[&block.at].clone()).collect();
    out.extend(made);
    Some((body.with_blocks(out), moved))
}

fn _terminates(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Jump | Operation::Branch))
}

/// A move that claims none of the original bytes.
fn _copy(beside: &Insn, into: u32, out_of: u32, width: u32) -> Arc<Insn> {
    let at = beside.covers.map_or(beside.at, |covers| covers.0);
    let mut made = Insn::new(
        beside.at,
        Some((at, at)),
        Some(Semantics {
            name: Some("mov".to_owned()),
            dests: vec![Loc::Held(Held { value: into, width })],
            sources: vec![Loc::Held(Held { value: out_of, width })],
            ..Semantics::new(Operation::Move)
        }),
        vec![into],
        vec![out_of],
    );
    made.op = beside.op.clone();
    Arc::new(made)
}

/// How wide each value is, from the widest operand naming it.
fn _widths(body: &LirBody) -> IndexMap<u32, u32> {
    let mut out: IndexMap<u32, u32> = IndexMap::default();
    let mut widen = |value: u32, width: u32| {
        let had = out.get(&value).copied().unwrap_or(0);
        out.insert(value, had.max(width));
    };
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        if let Some(what) = &one.what {
            for operand in what.dests.iter().chain(&what.sources) {
                for held in ir::values(operand) {
                    widen(held.value, held.width);
                }
            }
        }
        for (held, _register) in one.requires.iter().chain(&one.delivers) {
            widen(held.value, held.width);
        }
        for (named, wide) in &one.widths {
            widen(*named, *wide);
        }
    }
    out
}

fn _renamed(one: &Arc<Insn>, rename: &IndexMap<u32, u32>) -> Arc<Insn> {
    if rename.is_empty() { Arc::clone(one) } else { spiller::_renamed(one, rename) }
}

pub fn _next_value(body: &LirBody) -> u32 {
    let mut seen: BTreeSet<u32> = BTreeSet::from([0]);
    for block in &body.blocks {
        seen.extend(block.arrives());
        for one in &block.insns {
            seen.extend(one.defines.iter().copied());
            seen.extend(one.uses.iter().copied());
        }
    }
    seen.last().copied().expect("seeded with zero") + 1
}

/// One block `value` is referenced in: LLVM's `SplitAnalysis::BlockInfo`.
struct UseBlock {
    block: i64,
    /// The instructions naming the value: each a memory operand once it spills.
    references: usize,
    live_in: bool,
    live_out: bool,
    /// Positions of the first and last instruction naming the value.
    first: usize,
    last: usize,
}

/// The blocks `value` is referenced in, and those it only passes through.
struct Analysis {
    uses: Vec<UseBlock>,
    through: Vec<i64>,
}

fn analysed(body: &LirBody, value: u32, live_in: &allocate::Live, live_out: &allocate::Live) -> Analysis {
    let mut uses = Vec::new();
    let mut through = Vec::new();
    for block in &body.blocks {
        let (entering, leaving) = (live_in[&block.at].contains(&value), live_out[&block.at].contains(&value));
        let arrives = block.arrives().contains(&value);
        let named: Vec<usize> = (0..block.insns.len())
            .filter(|at| block.insns[*at].defines.contains(&value) || block.insns[*at].uses.contains(&value))
            .collect();
        if !named.is_empty() || arrives {
            uses.push(UseBlock {
                block: block.at,
                references: named.len(),
                live_in: entering && !arrives,
                live_out: leaving,
                first: named.first().copied().unwrap_or(0),
                last: named.last().copied().unwrap_or(0),
            });
        } else if entering && leaving {
            through.push(block.at);
        }
    }
    Analysis { uses, through }
}

/// What holds a register where: the occupants' segments and the points that destroy it.
pub struct Occupied<'a> {
    pub segments: IndexMap<Register, Vec<Segment>>,
    pub masks: &'a allocate::Masks,
}

impl Occupied<'_> {
    /// The first and last slot inside `span` at which anything holds or
    /// destroys `register`.
    fn interference(&self, register: Register, width: u32, span: (i64, i64)) -> Option<(i64, i64)> {
        let whole = allocate::_whole(register);
        let inside = Segment { start: span.0, end: span.1 };
        let held = self
            .segments
            .get(&whole)
            .into_iter()
            .flatten()
            .filter(|one| one.overlaps(&inside))
            .map(|one| (one.start.max(span.0), one.end.min(span.1) - 1));
        let destroyed = self
            .masks
            .iter()
            .filter(|mask| {
                span.0 <= mask.slot
                    && mask.slot < span.1
                    && (mask.during.contains(&whole) || mask.before.contains(&whole) || (width > 2 && mask.high.contains(&whole)))
            })
            .map(|mask| (mask.slot, mask.slot));
        held.chain(destroyed).reduce(|one, other| (one.0.min(other.0), one.1.max(other.1)))
    }
}

/// The interference in a block, as the positions of the first and last
/// instruction it touches: -1 is the block's entry.
type Blocked<'a> = &'a dyn Fn(i64) -> Option<(i64, i64)>;

/// A block's interference slots as instruction positions.
fn _positions(index: &Indexes, block: i64, slots: (i64, i64)) -> (i64, i64) {
    let first = index.span[&block].0;
    let at = |slot: i64| (slot - first) / ranges::PER_INSN - 1;
    (at(slots.0), at(slots.1))
}

/// Where a split can last be placed in `block`: before its terminator.
fn _last_split(block: &LirBlock) -> i64 {
    let length = block.insns.len() as i64;
    if block.insns.last().is_some_and(|one| _terminates(one)) { length - 1 } else { length }
}

/// `addSplitConstraints`: use blocks want the value in a register at the
/// borders it is live across. Interference at a border must spill there;
/// interference between the border and the nearest use prefers to; any
/// other is split around inside the block. None when no bundle wants a
/// register.
fn _use_constraints(placement: &mut spillplacement::Placement, body: &LirBody, analysis: &Analysis, blocked: Blocked) -> Option<()> {
    let at_of: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut constraints = Vec::new();
    for one in &analysis.uses {
        let mut entry = if one.live_in { Border::PrefReg } else { Border::DontCare };
        let mut exit = if one.live_out { Border::PrefReg } else { Border::DontCare };
        if let Some((low, high)) = blocked(one.block) {
            if one.live_in {
                entry = if low < 0 { Border::MustSpill } else if low <= one.first as i64 { Border::PrefSpill } else { entry };
            }
            if one.live_out {
                let last = _last_split(at_of[&one.block]);
                exit = if high >= last { Border::MustSpill } else if high >= one.last as i64 { Border::PrefSpill } else { exit };
            }
        }
        // What a register saves here is a memory operand per reference: the
        // rest of a split value spills whole, with no local interval.
        constraints.push(Constraint { block: one.block, entry, exit, weight: one.references.max(1) as f64 });
    }
    placement.add_constraints(&constraints);
    placement.scan().then_some(())
}

/// `growRegion` and `addThroughConstraints`: take in the through blocks
/// around each bundle that turned positive. One without interference links
/// its bundles; one with it prefers the stack at each border, or must spill
/// where the interference reaches the border. With no register (`blocked`
/// is None) every through block prefers the stack, strongly.
fn _grown(placement: &mut spillplacement::Placement, body: &LirBody, analysis: &Analysis, blocked: Option<Blocked>) -> Vec<i64> {
    let at_of: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut todo: BTreeSet<i64> = analysis.through.iter().copied().collect();
    let mut active: Vec<i64> = Vec::new();
    loop {
        let added = active.len();
        for bundle in placement.recent.clone() {
            for block in &placement.bundles.blocks[bundle] {
                if todo.remove(block) {
                    active.push(*block);
                }
            }
        }
        if active.len() == added {
            return active;
        }
        let new = &active[added..];
        match blocked {
            Some(blocked) => {
                let mut open = Vec::new();
                let mut constraints = Vec::new();
                for block in new {
                    match blocked(*block) {
                        None => open.push(*block),
                        Some((low, high)) => constraints.push(Constraint {
                            block: *block,
                            entry: if low < 0 { Border::MustSpill } else { Border::PrefSpill },
                            exit: if high >= _last_split(at_of[block]) { Border::MustSpill } else { Border::PrefSpill },
                            weight: 1.0,
                        }),
                    }
                }
                placement.add_constraints(&constraints);
                placement.add_links(&open);
            }
            None => placement.add_pref_spill(new, true),
        }
        placement.iterate();
    }
}

/// The ranges of a block a piece holds, given which borders are in a
/// register, the first and last reference (None for a through block) and
/// the interference: entering in a register, the piece holds the value
/// until the interference or its last use; leaving in one, from the
/// interference or its first use.
fn _ranges_in(length: usize, entry: bool, exit: bool, named: Option<(usize, usize)>, interference: Option<(i64, i64)>) -> Vec<(usize, usize)> {
    let length = length as i64;
    let mut out = Vec::new();
    match interference {
        None if entry && exit => out.push((0, length)),
        None => {
            match named {
                Some((first, last)) => {
                    if entry {
                        out.push((0, last as i64 + 1));
                    }
                    if exit {
                        out.push((first as i64, length));
                    }
                }
                None if entry || exit => out.push((0, length)),
                None => {}
            }
        }
        Some((low, high)) => {
            if entry {
                let end = match named {
                    Some((_, last)) if !exit => low.min(last as i64 + 1),
                    _ => low,
                };
                out.push((0, end));
            }
            if exit {
                let start = match named {
                    Some((first, _)) if !entry => (high + 1).max(first as i64),
                    _ => high + 1,
                };
                out.push((start, length));
            }
        }
    }
    out.into_iter().filter(|(from, to)| from < to).map(|(from, to)| (from as usize, to as usize)).collect()
}

/// What a candidate's register bundles hold, block by block.
fn _held(placement: &spillplacement::Placement, body: &LirBody, analysis: &Analysis, through: &[i64], live: &BTreeSet<usize>, blocked: Blocked) -> Region {
    let length: IndexMap<i64, usize> = body.blocks.iter().map(|block| (block.at, block.insns.len())).collect();
    let borders = |block: i64, live_in: bool, live_out: bool| {
        let (entry, exit) = placement.bundles.of[&block];
        (live_in && live.contains(&entry), live_out && live.contains(&exit))
    };
    let mut region = Region::default();
    for one in &analysis.uses {
        let (entry, exit) = borders(one.block, one.live_in, one.live_out);
        for (from, to) in _ranges_in(length[&one.block], entry, exit, Some((one.first, one.last)), blocked(one.block)) {
            region.add(one.block, from, to);
        }
    }
    for block in through {
        let (entry, exit) = borders(*block, true, true);
        for (from, to) in _ranges_in(length[block], entry, exit, None, blocked(*block)) {
            region.add(*block, from, to);
        }
    }
    region
}

/// What holding `region` in a register saves: a memory operand per
/// reference inside it, less every copy `crossings` says carving it adds.
fn _benefit(body: &LirBody, value: u32, region: &Region, frequency: &IndexMap<i64, f64>, live: (&allocate::Live, &allocate::Live)) -> f64 {
    let saved: f64 = body
        .blocks
        .iter()
        .map(|block| {
            let covered = (0..block.insns.len())
                .filter(|at| region.covers(block.at, *at))
                .filter(|at| block.insns[*at].defines.contains(&value) || block.insns[*at].uses.contains(&value))
                .count();
            covered as f64 * frequency[&block.at]
        })
        .sum();
    let copies: f64 = crossings(body, value, region, live.0, live.1)
        .iter()
        .map(|(at, _)| match at {
            Crossing::Inside { block, .. } => frequency[block],
            Crossing::Edge { from, to } => frequency[from].min(frequency[to]),
        })
        .sum();
    saved - copies
}

/// Whether holding `region` in a register saves more than its copies cost,
/// weighing each block by its loop depth.
pub fn pays(body: &LirBody, value: u32, region: &Region, live: (&allocate::Live, &allocate::Live)) -> bool {
    let frequency: IndexMap<i64, f64> = ranges::depths(body).into_iter().map(|(at, depth)| (at, ranges::level(depth))).collect();
    _benefit(body, value, region, &frequency, live) > 0.0
}

/// Whether `region` holds all of `value`'s range.
fn _whole_range(body: &LirBody, value: u32, region: &Region, live: (&allocate::Live, &allocate::Live)) -> bool {
    body.blocks.iter().all(|block| {
        let named = block.insns.iter().any(|one| one.defines.contains(&value) || one.uses.contains(&value));
        let across = live.0[&block.at].contains(&value) || live.1[&block.at].contains(&value);
        !(named || across) || region.spans.get(&block.at) == Some(&vec![(0, block.insns.len())])
    })
}

/// `tryRegionSplit` and `doRegionSplit`: the regions of `value` to hold in
/// registers. The best candidate register's region, placed by block
/// frequency against its interference, comes first; the compact region its
/// use clusters form takes the uses left over. Each region saves more than
/// its copies cost, and none is the whole range.
#[allow(clippy::too_many_arguments)]
pub fn placed(
    body: &LirBody,
    value: u32,
    index: &Indexes,
    live: (&allocate::Live, &allocate::Live),
    bundles: &spillplacement::Bundles,
    candidates: &[Register],
    occupied: &Occupied,
    width: u32,
) -> Vec<Region> {
    let analysis = analysed(body, value, live.0, live.1);
    if analysis.uses.is_empty() {
        return Vec::new();
    }
    let mut placement = spillplacement::Placement::new(body, bundles);
    let benefit = |region: &Region, placement: &spillplacement::Placement| {
        if region.is_empty() || _whole_range(body, value, region, live) {
            return 0.0;
        }
        _benefit(body, value, region, &placement.frequency, live)
    };
    let open = |_: i64| None;
    let mut compact = Region::default();
    if !analysis.through.is_empty() {
        placement.prepare();
        if _use_constraints(&mut placement, body, &analysis, &open).is_some() {
            let through = _grown(&mut placement, body, &analysis, None);
            let live = placement.finish();
            compact = _held(&placement, body, &analysis, &through, &live, &open);
        }
    }
    let mut best: Option<(f64, Region)> = None;
    let mut tried = BTreeSet::new();
    for register in candidates {
        if !tried.insert(allocate::_whole(*register)) {
            continue;
        }
        let blocked = |block: i64| occupied.interference(*register, width, index.span[&block]).map(|slots| _positions(index, block, slots));
        placement.prepare();
        if _use_constraints(&mut placement, body, &analysis, &blocked).is_none() {
            continue;
        }
        let through = _grown(&mut placement, body, &analysis, Some(&blocked));
        let live = placement.finish();
        let held = _held(&placement, body, &analysis, &through, &live, &blocked);
        let saved = benefit(&held, &placement);
        if saved > 0.0 && best.as_ref().is_none_or(|(found, _)| saved > *found) {
            best = Some((saved, held));
        }
    }
    let mut regions = Vec::new();
    if let Some((_, held)) = best {
        compact.spans.retain(|block, _| !held.spans.contains_key(block));
        regions.push(held);
    }
    if benefit(&compact, &placement) > 0.0 {
        regions.push(compact);
    }
    regions
}

/// `tryLocalSplit`, for a value live in one block: the longest run of its
/// references some candidate register is free across, when that is at
/// least two and not all of them.
pub fn local(body: &LirBody, value: u32, index: &Indexes, live: (&allocate::Live, &allocate::Live), candidates: &[Register], occupied: &Occupied, width: u32) -> Option<Region> {
    let analysis = analysed(body, value, live.0, live.1);
    let [one] = analysis.uses.as_slice() else { return None };
    if one.live_in || one.live_out || !analysis.through.is_empty() {
        return None;
    }
    let block = body.blocks.iter().find(|block| block.at == one.block)?;
    let named: Vec<usize> = (0..block.insns.len())
        .filter(|at| block.insns[*at].defines.contains(&value) || block.insns[*at].uses.contains(&value))
        .collect();
    let first = index.span[&block.at].0;
    let slot = |position: usize| first + ranges::PER_INSN * (position as i64 + 1);
    let mut best: Option<(usize, usize, usize, usize)> = None;
    let mut tried = BTreeSet::new();
    for register in candidates {
        if !tried.insert(allocate::_whole(*register)) {
            continue;
        }
        let mut start = 0;
        while start < named.len() {
            let mut end = start;
            while end + 1 < named.len() && occupied.interference(*register, width, (slot(named[start]), slot(named[end + 1]) + ranges::PER_INSN)).is_none() {
                end += 1;
            }
            let count = end - start + 1;
            let span = named[end] - named[start];
            if count >= 2 && count < named.len() && best.is_none_or(|(had, wide, _, _)| (count, Reverse(span)) > (had, Reverse(wide))) {
                best = Some((count, span, named[start], named[end] + 1));
            }
            start = end + 1;
        }
    }
    let (_, _, from, to) = best?;
    let mut region = Region::default();
    region.add(block.at, from, to);
    Some(region)
}

/// `tryBlockSplit`: each block naming the value at least twice holds it
/// from its first reference to its last; the rest spills.
pub fn per_block(body: &LirBody, value: u32, live: (&allocate::Live, &allocate::Live)) -> Vec<Region> {
    let analysis = analysed(body, value, live.0, live.1);
    if analysis.uses.len() + analysis.through.len() < 2 {
        return Vec::new();
    }
    analysis
        .uses
        .iter()
        .filter(|one| one.references >= 2)
        .map(|one| {
            let mut region = Region::default();
            region.add(one.block, one.first, one.last + 1);
            region
        })
        .collect()
}

/// `value`'s interval divided at `region`: the piece inside and the rest,
/// each weighted as `intervals::weights` weighs a whole range.
pub fn divided(body: &LirBody, index: &Indexes, whole: &Interval, region: &Region, fresh: u32) -> (Interval, Interval) {
    let spans = region.segments(body, index);
    let (mut inside, mut outside) = (Vec::new(), Vec::new());
    for one in &whole.segments {
        let mut cursor = one.start;
        let mut cuts: Vec<Segment> = spans.iter().filter(|span| span.overlaps(one)).copied().collect();
        cuts.sort_by_key(|span| span.start);
        for span in cuts {
            let (start, end) = (span.start.max(one.start), span.end.min(one.end));
            if cursor < start {
                outside.push(Segment { start: cursor, end: start });
            }
            inside.push(Segment { start, end });
            cursor = end;
        }
        if cursor < one.end {
            outside.push(Segment { start: cursor, end: one.end });
        }
    }
    let deep = ranges::depths(body);
    let (mut within, mut without) = (0.0, 0.0);
    for block in &body.blocks {
        let each = ranges::level(deep.get(&block.at).copied().unwrap_or(0));
        for (at, one) in block.insns.iter().enumerate() {
            if !(one.defines.contains(&whole.value) || one.uses.contains(&whole.value)) {
                continue;
            }
            if region.covers(block.at, at) {
                within += each;
            } else {
                without += each;
            }
        }
    }
    let weighed = |value: u32, segments: Vec<Segment>, references: f64| {
        let mut made = Interval::new(value, ranges::_merged(segments));
        made.weight = references / (made.size() + ranges::GRACE) as f64;
        made
    };
    (weighed(fresh, inside, within), weighed(whole.value, outside, without))
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_splitkit.py`.

    use std::collections::BTreeSet;
    use crate::support::hash::HashMap;
    use std::sync::Arc;

    use iced_x86::Register;
    use crate::support::hash::IndexMap;

    use super::{carved, loop_bases, Region};
    use crate::analysis::intervals::{self, Indexes};
    use crate::model::ir::{Addr, Held, Imm, Loc, Mem, Operation, Semantics, Space};
    use crate::model::lir::{Insn, LirBlock, LirBody};

    fn held(value: u32, width: u32) -> Loc {
        Loc::Held(Held { value, width })
    }

    fn imm(value: i64) -> Loc {
        Loc::Imm(Imm { value, width: 2, address: None })
    }

    fn sem(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>, target: Option<i64>) -> Semantics {
        Semantics { name: Some(name.to_owned()), dests, sources, target, ..Semantics::new(op) }
    }

    fn _insn(at: i64, what: Semantics, defines: &[u32], uses: &[u32]) -> Arc<Insn> {
        Arc::new(Insn::new(at, Some((at, at + 2)), Some(what), defines.to_vec(), uses.to_vec()))
    }

    fn jump(at: i64, to: i64) -> Arc<Insn> {
        _insn(at, sem(Operation::Jump, "jmp", vec![], vec![], Some(to)), &[], &[])
    }

    fn move_imm(at: i64, into: u32, value: i64) -> Arc<Insn> {
        _insn(at, sem(Operation::Move, "mov", vec![held(into, 2)], vec![imm(value)], None), &[into], &[])
    }

    fn push(at: i64, value: u32) -> Arc<Insn> {
        _insn(at, sem(Operation::Push, "push", vec![], vec![held(value, 2)], None), &[], &[value])
    }

    fn add(at: i64, value: u32) -> Arc<Insn> {
        let what = sem(Operation::Binary, "add", vec![held(value, 2)], vec![held(value, 2), imm(1)], None);
        _insn(at, what, &[value], &[value])
    }

    fn block(at: i64, insns: Vec<Arc<Insn>>, succ: &[i64]) -> LirBlock {
        LirBlock { succ: succ.to_vec(), ..LirBlock::new(at, insns) }
    }

    fn body(name: &str, blocks: Vec<LirBlock>) -> LirBody {
        LirBody::new(name, 0, blocks, IndexMap::default(), IndexMap::default())
    }

    fn where_() -> Addr {
        Addr { base: Register::SI, ..Addr::new(Space::Segment, 0x10) }
    }

    /// v3 is made before the loop, read in it and after it, as a cell's base.
    fn _pointer_across_a_loop() -> LirBody {
        let cell = Loc::Mem(Mem {
            through: Register::None,
            offset: 0,
            disp_width: 2,
            base: Some(Held { value: 3, width: 2 }),
            ..Mem::new(Some(where_()), 2)
        });
        body(
            "one",
            vec![
                block(0, vec![move_imm(0, 3, 0x40), move_imm(2, 4, 1), jump(4, 0x10)], &[0x10]),
                block(
                    0x10,
                    vec![
                        add(0x10, 4),
                        _insn(0x11, sem(Operation::Move, "mov", vec![held(6, 2)], vec![cell.clone()], None), &[6], &[3]),
                        _insn(0x12, sem(Operation::Branch, "jne", vec![], vec![], Some(0x10)), &[], &[]),
                    ],
                    &[0x10, 0x20],
                ),
                block(
                    0x20,
                    vec![
                        _insn(0x20, sem(Operation::Move, "mov", vec![held(5, 2)], vec![cell], None), &[5], &[3]),
                        _insn(0x22, sem(Operation::Return, "ret", vec![], vec![], None), &[], &[5]),
                    ],
                    &[],
                ),
            ],
        )
    }

    fn region(body: &LirBody, blocks: &[i64]) -> Region {
        Region::blocks(body, blocks.iter().copied())
    }

    #[test]
    fn test_a_piece_leaves_its_region_after_its_last_use() {
        let body = body(
            "one",
            vec![
                block(0, vec![move_imm(0, 3, 1), jump(2, 0x10)], &[0x10]),
                block(0x10, vec![add(0x10, 3), move_imm(0x12, 7, 2), jump(0x14, 0x20)], &[0x20]),
                block(0x20, vec![push(0x20, 3), push(0x21, 7)], &[]),
            ],
        );
        let cut = carved(&body, 3, 9, 2, &region(&body, &[0x10])).expect("cut");
        let live = intervals::intervals(&cut, None);
        assert!(!live[&9].overlaps(&live[&7]));
    }

    fn read(values: &HashMap<u32, i64>, operand: &Loc) -> i64 {
        match operand {
            Loc::Imm(one) => one.value,
            Loc::Held(one) => values[&one.value],
            other => panic!("{other:?}"),
        }
    }

    fn into(what: &Semantics) -> u32 {
        match &what.dests[0] {
            Loc::Held(one) => one.value,
            other => panic!("{other:?}"),
        }
    }

    /// What the pushes along `path` write, running moves, adds and pushes.
    fn _pushed(body: &LirBody, path: &[i64]) -> Vec<i64> {
        let mut values: HashMap<u32, i64> = HashMap::default();
        let mut pushed = Vec::new();
        let blocks: HashMap<i64, &LirBlock> = body.blocks.iter().map(|one| (one.at, one)).collect();
        for at in path {
            for one in &blocks[at].insns {
                let what = one.what.as_ref().expect("semantics");
                match what.op {
                    Operation::Move => {
                        let value = read(&values, &what.sources[0]);
                        values.insert(into(what), value);
                    }
                    Operation::Binary => {
                        let value = read(&values, &what.sources[0]) + read(&values, &what.sources[1]);
                        values.insert(into(what), value);
                    }
                    Operation::Push => pushed.push(read(&values, &what.sources[0])),
                    _ => {}
                }
            }
        }
        pushed
    }

    #[test]
    fn test_a_region_block_also_reached_from_inside_it_keeps_the_inside_value() {
        let branch = _insn(2, sem(Operation::Branch, "jne", vec![], vec![], Some(0x20)), &[], &[]);
        let body = body(
            "one",
            vec![
                block(0, vec![move_imm(0, 3, 1), branch], &[0x10, 0x20]),
                block(0x10, vec![add(0x10, 3), jump(0x12, 0x20)], &[0x20]),
                block(0x20, vec![push(0x20, 3)], &[]),
            ],
        );
        let cut = carved(&body, 3, 9, 2, &region(&body, &[0x10, 0x20])).expect("cut");
        assert_eq!(_pushed(&cut, &[0, 0x10, 0x20]), vec![2]);
        assert_eq!(_pushed(&cut, &[0, 0x20]), vec![1]);
    }

    #[test]
    fn test_a_cut_range_renames_what_an_instruction_requires_and_delivers() {
        let what = sem(Operation::Call, "call", vec![], vec![], None);
        let call = Insn {
            delivers: vec![(Held { value: 3, width: 2 }, Register::DI)],
            widths: vec![(3, 2)],
            ..Insn::new(0x10, Some((0x10, 0x12)), Some(what), vec![3], vec![])
        };
        let out = Insn {
            requires: vec![(Held { value: 3, width: 1 }, Register::AL)],
            widths: vec![(3, 1)],
            ..Insn::new(0x12, Some((0x12, 0x13)), None, vec![], vec![3])
        };
        let body = body(
            "one",
            vec![
                block(0, vec![jump(0, 0x10)], &[0x10]),
                block(0x10, vec![Arc::new(call), Arc::new(out), jump(0x14, 0x20)], &[0x20]),
                block(0x20, vec![push(0x20, 3)], &[]),
            ],
        );
        let cut = carved(&body, 3, 9, 2, &region(&body, &[0x10])).expect("cut");
        for one in cut.insns() {
            let named: BTreeSet<u32> = one.uses.iter().chain(&one.defines).copied().collect();
            assert!(one.requires.iter().all(|(held, _)| one.uses.contains(&held.value)), "{one:?}");
            assert!(one.delivers.iter().all(|(held, _)| one.defines.contains(&held.value)), "{one:?}");
            assert!(one.widths.iter().all(|(value, _)| named.contains(value)), "{one:?}");
        }
    }

    #[test]
    fn test_a_cut_range_renames_the_cell_it_is_the_base_of() {
        let loop_ = _pointer_across_a_loop();
        let body = carved(&loop_, 3, 9, 2, &region(&loop_, &[0x10])).expect("cut");
        let loads: Vec<Arc<Insn>> = body
            .insns()
            .into_iter()
            .filter(|one| one.what.as_ref().is_some_and(|what| what.sources.iter().any(|x| matches!(x, Loc::Mem(_)))))
            .collect();
        assert!(loads.iter().any(|one| one.uses != [3]), "nothing was cut; the fixture does not reach the rename");
        let mut last = None;
        for load in &loads {
            let Loc::Mem(cell) = &load.what.as_ref().expect("semantics").sources[0] else { panic!("not a cell") };
            let base = cell.base.expect("base");
            assert_eq!(load.uses, [base.value], "uses {:?}, cell on {base:?}", load.uses);
            assert_eq!(cell.through, Register::None, "the rename placed it");
            last = Some(cell.clone());
        }
        let cell = last.expect("a load");
        assert_eq!((cell.addr, cell.width, cell.offset, cell.disp_width), (Some(where_()), 2, 0, 2));
    }

    #[test]
    fn test_a_loop_scoped_base_piece_enters_once_and_leaves_after_the_loop() {
        let (cut, kept) = loop_bases(&_pointer_across_a_loop(), &BTreeSet::from([3]));
        assert_eq!(kept.len(), 1);
        let fresh = *kept.iter().next().expect("one");
        let blocks: HashMap<i64, &LirBlock> = cut.blocks.iter().map(|one| (one.at, one)).collect();
        // v3 is a constant: the piece remakes it on entry rather than copy it.
        assert!(blocks[&0].insns.iter().any(|one| one.defines == [fresh] && one.uses.is_empty()));
        // The loop never writes v3, so v3 still holds it after: no copy back.
        assert!(!cut.insns().iter().any(|one| one.defines == [3] && one.uses == [fresh]));
        let loop_cells: Vec<&Mem> = blocks[&0x10]
            .insns
            .iter()
            .filter_map(|one| one.what.as_ref())
            .flat_map(|what| what.dests.iter().chain(&what.sources))
            .filter_map(|place| if let Loc::Mem(cell) = place { Some(cell) } else { None })
            .collect();
        let bases: BTreeSet<u32> = loop_cells.iter().map(|one| one.base.expect("base").value).collect();
        assert!(!loop_cells.is_empty() && bases == BTreeSet::from([fresh]));
    }

    /// Only bases were carved: FRACTALEFFECT reloaded a spilled selector every trip.
    #[test]
    fn test_a_spilled_index_or_selector_is_carved_into_the_loop_like_a_base() {
        for role in ["index", "selector"] {
            let mut moved = _pointer_across_a_loop();
            for insn in moved.blocks.iter_mut().flat_map(|one| &mut one.insns) {
                let mut changed = (**insn).clone();
                if let Some(what) = changed.what.as_mut() {
                    for place in what.dests.iter_mut().chain(what.sources.iter_mut()) {
                        if let Loc::Mem(cell) = place {
                            let operand = cell.base.take();
                            if role == "index" {
                                cell.index = operand;
                            } else {
                                cell.selector = operand;
                            }
                        }
                    }
                }
                *insn = std::sync::Arc::new(changed);
            }
            let (_cut, kept) = loop_bases(&moved, &BTreeSet::from([3]));
            assert_eq!(kept.len(), 1, "{role}");
        }
    }

    /// A value whose register is taken before and after a loop, but free in
    /// it, spilled whole and reloaded every trip. Placement keeps it in the
    /// register across the loop, entering after the interference before it
    /// and leaving before the interference after it.
    #[test]
    fn test_a_register_free_only_in_the_loop_holds_the_value_there() {
        let body = _pointer_across_a_loop();
        let index = intervals::indexed(&body);
        let live = crate::backend::allocate::live(&body);
        let bundles = crate::backend::spillplacement::bundles(&body);
        let slot = |block: i64, position: i64| index.span[&block].0 + intervals::PER_INSN * (position + 1);
        // Taken between the two definitions before the loop, and after the read behind it.
        let taken = vec![
            intervals::Segment { start: slot(0, 0) + 1, end: slot(0, 2) },
            intervals::Segment { start: slot(0x20, 1), end: index.span[&0x20].1 },
        ];
        let masks = Vec::new();
        let occupied = super::Occupied { segments: IndexMap::from_iter([(Register::EAX, taken)]), masks: &masks };
        let regions = super::placed(&body, 3, &index, (&live.0, &live.1), &bundles, &[Register::AX], &occupied, 2);
        let first = regions.first().expect("a region");
        assert_eq!(first.spans.get(&0x10), Some(&vec![(0, 3)]), "{regions:?}");
        assert_eq!(first.spans.get(&0x20), Some(&vec![(0, 1)]), "{regions:?}");
    }

    /// A range in one block whose register is taken partway spilled whole;
    /// the local split keeps the longest run of references it is free across.
    #[test]
    fn test_a_local_split_holds_the_references_before_the_interference() {
        let body = body(
            "one",
            vec![block(0, vec![move_imm(0, 3, 1), add(2, 3), add(4, 3), move_imm(6, 7, 2), push(8, 3), push(10, 7)], &[])],
        );
        let index = intervals::indexed(&body);
        let live = crate::backend::allocate::live(&body);
        let slot = |position: i64| index.span[&0].0 + intervals::PER_INSN * (position + 1);
        let taken = vec![intervals::Segment { start: slot(3), end: slot(5) }];
        let masks = Vec::new();
        let occupied = super::Occupied { segments: IndexMap::from_iter([(Register::EAX, taken)]), masks: &masks };
        let got = super::local(&body, 3, &index, (&live.0, &live.1), &[Register::AX], &occupied, 2).expect("a split");
        assert_eq!(got.spans.get(&0), Some(&vec![(0, 3)]));
    }

    /// A region that only reads the value copied it back anyway: a store per
    /// exit for a value its slot already held (the PARTICLE loop).
    #[test]
    fn test_a_piece_that_only_reads_its_value_copies_nothing_back() {
        let cut = carved(&_pointer_across_a_loop(), 3, 9, 2, &region(&_pointer_across_a_loop(), &[0x10])).expect("cut");
        assert!(!cut.insns().iter().any(|one| one.defines == [3] && one.uses == [9]));
    }

    #[test]
    fn test_a_loop_piece_restores_only_on_its_exiting_edge() {
        let mut body = _pointer_across_a_loop();
        let found = body.blocks.iter_mut().find(|one| one.at == 0x10).expect("loop");
        let mut insns = found.insns.clone();
        insns.insert(1, add(0x10, 3));
        *found = found.with_insns(insns);
        let cut = carved(&body, 3, 9, 2, &region(&body, &[0x10])).expect("cut");
        let found = cut.blocks.iter().find(|one| one.at == 0x10).expect("loop");
        assert!(!found.insns.iter().any(|one| one.defines == [3] && one.uses == [9]));
        let exit = cut.blocks.iter().find(|one| one.at == 0x20).expect("exit");
        assert!(exit.insns[0].defines == [3] && exit.insns[0].uses == [9]);
    }

    /// v4 counts to 10: set before the loop, tested in its header, bumped in its latch, read after.
    fn _counting_loop() -> LirBody {
        let compare = sem(Operation::Compare, "cmp", vec![], vec![held(4, 2), imm(10)], None);
        body(
            "count",
            vec![
                block(0, vec![move_imm(0, 4, 0), jump(2, 0x10)], &[0x10]),
                block(
                    0x10,
                    vec![
                        _insn(0x10, compare, &[], &[4]),
                        _insn(0x12, sem(Operation::Branch, "jge", vec![], vec![], Some(0x30)), &[], &[]),
                    ],
                    &[0x20, 0x30],
                ),
                block(0x20, vec![add(0x20, 4), jump(0x22, 0x10)], &[0x10]),
                block(
                    0x30,
                    vec![
                        _insn(0x30, sem(Operation::Move, "mov", vec![held(5, 2)], vec![held(4, 2)], None), &[5], &[4]),
                        _insn(0x32, sem(Operation::Return, "ret", vec![], vec![], None), &[], &[5]),
                    ],
                    &[],
                ),
            ],
        )
    }

    /// Values after executing the body: moves, add, cmp, jge, jmp and ret.
    fn _run(body: &LirBody) -> HashMap<u32, i64> {
        let blocks: HashMap<i64, &LirBlock> = body.blocks.iter().map(|one| (one.at, one)).collect();
        let mut values: HashMap<u32, i64> = HashMap::default();
        let (mut at, mut flags) = (body.entry, 0);
        for _step in 0..1000 {
            let (block, mut following) = (blocks[&at], None);
            for one in &block.insns {
                let what = one.what.as_ref().expect("semantics");
                match what.op {
                    Operation::Move => {
                        let value = read(&values, &what.sources[0]);
                        values.insert(into(what), value);
                    }
                    Operation::Binary => {
                        let value = read(&values, &what.sources[0]) + read(&values, &what.sources[1]);
                        values.insert(into(what), value);
                    }
                    Operation::Compare => flags = read(&values, &what.sources[0]) - read(&values, &what.sources[1]),
                    Operation::Branch => {
                        following = if flags >= 0 {
                            what.target
                        } else {
                            block.succ.iter().copied().find(|one| Some(*one) != what.target)
                        };
                    }
                    Operation::Jump => following = what.target,
                    Operation::Return => return values,
                    _ => {}
                }
            }
            at = following.unwrap_or(block.succ[0]);
        }
        panic!("the loop never ended");
    }

    #[test]
    fn test_a_cut_loop_counter_keeps_its_latch_value() {
        assert_eq!(_run(&_counting_loop())[&5], 10);
        let counting = _counting_loop();
        let body = carved(&counting, 4, 9, 2, &region(&counting, &[0x10, 0x20])).expect("cut");
        assert!(
            body.insns()
                .iter()
                .any(|one| [0x10, 0x20].contains(&one.at) && !one.uses.is_empty() && !one.uses.contains(&4)),
            "nothing was cut"
        );
        assert_eq!(_run(&body)[&5], 10);
    }
}
