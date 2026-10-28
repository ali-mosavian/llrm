//! Tail merging of a placed body: LLVM's `BranchFolder::TailMergeBlocks`.
//!
//! # How it works
//! The body is in `_placed`'s form, every fall-through an explicit jump. Blocks
//! with no successor, then the predecessors of each block with two or more, are
//! candidates; a predecessor is seen with its jump to that block taken off and
//! a branch to it reversed (LLVM strips and reverses the real ones, then puts
//! them back: here nothing is touched until a merge). Candidates are sorted by
//! a hash of their last instruction, the pairs of one hash compared back from
//! the end, and the blocks that share the longest tail merged: one keeps the
//! tail (split off its block if none is only the tail) and the others end in a
//! jump to it.
//!
//! # Deviations
//! See `docs/architecture/jumps-tailmerge-check.md`.
//!
//! # See also
//! `BranchFolding.cpp` at llvm-project d1106deb71cc: `TailMergeBlocks` L1109,
//! `TryTailMergeBlocks` L981, `ComputeSameTails` L743, `ProfitableToMerge`
//! L633, `ComputeCommonTailLength` L417, `CreateCommonTailOnlyBlock` L799,
//! `SplitMBBAt` L485, `setCommonTailEdgeWeights` L1259.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::analysis::frequency::Frequency;
use crate::analysis::loops as loopy;
use crate::backend::layout::_OPPOSITE;
use crate::backend::masm;
use crate::model::ir::{Operation, Semantics};
use crate::model::lir::{BlockOdds, Insn, LirBlock, LirBody};
use crate::support::hash::{HashMap, HashSet, IndexMap};

/// `TailMergeThreshold`: the most predecessors looked at together.
const THRESHOLD: usize = 150;

/// One instruction of a candidate's tail, with where it is in its block.
#[derive(Clone)]
struct Item {
    insn: Arc<Insn>,
    /// Position in the block's instructions.
    index: usize,
    /// Equal to nothing: it owns source bytes, or is something a copy cannot
    /// stand for.
    fixed: bool,
}

/// The instructions that count, with the ways out of the block taken as LLVM
/// sees them.
struct Seq {
    items: Vec<Item>,
    /// Branches left at the end (`CountTerminators`).
    terms: usize,
}

struct Pot {
    hash: u64,
    at: i64,
    seq: Seq,
}

/// A block whose tail is the common one, and where in its items it starts.
struct Same {
    pot: usize,
    start: usize,
    at: i64,
}

fn mergeable(one: &Insn) -> bool {
    let Some(what) = &one.what else {
        return false;
    };
    one.inserted()
        && !matches!(what.op, Operation::Barrier | Operation::Data)
        && one.symbol != Some(true)
        && one.group.is_none()
}

/// `isIdenticalTo`: the same operation on the same registers, ways out and
/// frame effect; the values are the allocator's names for them and are
/// renamed to one another after.
fn identical(
    one: &Item,
    other: &Item,
    calls: &IndexMap<i64, String>,
) -> bool {
    if one.fixed || other.fixed {
        return false;
    }
    let (a, b) = (&one.insn, &other.insn);
    // The callee is not in the instruction: it is the routine the call's
    // position names (`Procedure::callees`).
    if a.what.as_ref().is_some_and(|what| what.op == Operation::Call) {
        match (calls.get(&a.at), calls.get(&b.at)) {
            (Some(x), Some(y)) if x == y => {}
            _ => return false,
        }
    }
    a.what == b.what
        && a.clobbers == b.clobbers
        && a.clobbers_high == b.clobbers_high
        && a.frame_adjust == b.frame_adjust
        && a.requires.len() == b.requires.len()
        && a.requires.iter().zip(&b.requires).all(|(x, y)| x.1 == y.1)
        && a.delivers.len() == b.delivers.len()
        && a.delivers.iter().zip(&b.delivers).all(|(x, y)| x.1 == y.1)
}

/// `HashMachineInstr` of the last instruction, from its operation and the
/// registers it names.
fn hash_of(one: &Insn) -> u64 {
    let mut hasher = rustc_hash::FxHasher::default();
    one.what.hash(&mut hasher);
    one.frame_adjust.hash(&mut hasher);
    for (_value, register) in &one.requires {
        register.hash(&mut hasher);
    }
    for (_value, register) in &one.delivers {
        register.hash(&mut hasher);
    }
    hasher.finish()
}

fn jump_to(
    at: i64,
    target: i64,
) -> Arc<Insn> {
    Arc::new(Insn::new(
        at,
        Some((at, at)),
        Some(Semantics { name: Some("jmp".into()), target: Some(target), ..Semantics::new(Operation::Jump) }),
        Vec::new(),
        Vec::new(),
    ))
}

/// The items of `block`, and for a candidate with a block `onward` to reach,
/// the form LLVM compares it in: the jump to `onward` taken off, a branch to
/// `onward` reversed to the way it did not go. `None` where LLVM's
/// `analyzeBranch` or `reverseBranchCondition` would fail.
fn seq_of(
    block: &LirBlock,
    onward: Option<i64>,
) -> Option<Seq> {
    seq_from(block, 0, onward)
}

/// The item `insn` at `index` makes, if it counts.
fn item_of(
    insn: &Arc<Insn>,
    index: usize,
) -> Option<Item> {
    let nop =
        insn.what.as_ref().is_some_and(|what| what.op == Operation::Nothing && what.name.as_deref() == Some("nop"));
    // An anchor emits nothing: LLVM has none after allocation to count.
    if insn.is_meta() || nop || super::_inert(insn) {
        return (!insn.owned().is_empty()).then(|| Item { insn: Arc::clone(insn), index, fixed: true });
    }
    Some(Item { insn: Arc::clone(insn), index, fixed: !mergeable(insn) })
}

/// Where the end of `block` is seen from: the last three items, a jump, the
/// branch before it and the instruction before that, are all that
/// `HashEndOfMBB` and `CountTerminators` read.
fn end_of(block: &LirBlock) -> usize {
    let mut seen = 0;
    for (index, insn) in block.insns.iter().enumerate().rev() {
        seen += usize::from(item_of(insn, index).is_some());
        if seen == 3 {
            return index;
        }
    }
    0
}

/// `seq_of` of the instructions from `from` on, indexed as in `block`.
fn seq_from(
    block: &LirBlock,
    from: usize,
    onward: Option<i64>,
) -> Option<Seq> {
    let mut items = Vec::new();
    for (index, insn) in block.insns.iter().enumerate().skip(from) {
        #[cfg(test)]
        ITEMS_BUILT.with(|one| one.set(one.get() + 1));
        items.extend(item_of(insn, index));
    }
    let mut terms = 0;
    if let Some(onward) = onward {
        let jump = items.pop()?;
        let what = jump.insn.what.as_ref()?;
        if what.op != Operation::Jump || what.indirect || !what.table.is_empty() || !jump.insn.inserted() {
            return None;
        }
        let way = what.target?;
        match items.last() {
            Some(last) if last.insn.what.as_ref().is_some_and(|branch| branch.op == Operation::Branch) => {
                let branch = last.insn.what.as_ref().expect("checked");
                if branch.indirect || branch.target.is_none() {
                    return None;
                }
                let taken = branch.target.expect("checked");
                if way == onward && taken != onward {
                    terms = 1;
                } else if taken == onward && way != onward {
                    let opposite = _OPPOSITE.get(branch.name.as_deref()?)?;
                    let at = items.len() - 1;
                    let flipped = Arc::new(Insn {
                        what: Some(Semantics { name: Some((*opposite).into()), target: Some(way), ..branch.clone() }),
                        ..(*items[at].insn).clone()
                    });
                    items[at].insn = flipped;
                    terms = 1;
                } else {
                    return None;
                }
            }
            _ => {
                if way != onward {
                    return None;
                }
            }
        }
    }
    Some(Seq { items, terms })
}

/// `ComputeCommonTailLength`.
fn common(
    one: &Seq,
    other: &Seq,
    calls: &IndexMap<i64, String>,
) -> usize {
    one.items.iter().rev().zip(other.items.iter().rev()).take_while(|(x, y)| identical(x, y, calls)).count()
}

struct Folder {
    /// The routine each call goes to, by position.
    calls: IndexMap<i64, String>,
    order: Vec<crate::model::lir::Block>,
    pos: IndexMap<i64, usize>,
    odds: BlockOdds,
    /// How often each block of `source` runs, worked out when a merged tail
    /// ends in a branch.
    frequency: Option<Arc<Frequency>>,
    /// The block of `source` a block made by a split was cut from.
    origin: IndexMap<i64, i64>,
    /// Tails kept whole: the table of frequencies no longer says how often
    /// they run.
    kept: Vec<i64>,
    /// Innermost loop header by block.
    inner: Option<IndexMap<i64, i64>>,
    headers: HashSet<i64>,
    /// The blocks on a cycle, natural loop or not.
    cyclic: HashSet<i64>,
    /// The body as it came: what `runs` and the loops are of.
    source: LirBody,
    tried: HashSet<i64>,
    /// The blocks changed since the predecessor map was last made current, with
    /// the successors they had (`None` for a block made since).
    touched: IndexMap<i64, Option<Vec<i64>>>,
    rename: IndexMap<u32, u32>,
    next: i64,
    minimum: usize,
    size: bool,
    entry: i64,
    /// The candidates now being merged: seen without their jump to `onward`.
    stripped: HashSet<i64>,
    changed: bool,
}

#[cfg(test)]
thread_local! {
    pub(crate) static ITEMS_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(crate) static SELF_LOOP_SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(crate) static PREDECESSOR_MAPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The blocks on a cycle: members of a component of two or more, or a block
/// that jumps to itself.
pub(crate) fn cyclic_blocks(blocks: &[crate::model::lir::Block]) -> HashSet<i64> {
    let selfish: HashSet<i64> = blocks
        .iter()
        .inspect(|_| {
            #[cfg(test)]
            SELF_LOOP_SCANS.with(|one| one.set(one.get() + 1));
        })
        .filter(|block| block.succ.contains(&block.at))
        .map(|block| block.at)
        .collect();
    llrm_support::graph::components(blocks)
        .into_iter()
        .filter(|members| members.len() > 1 || selfish.contains(&members[0]))
        .flatten()
        .collect()
}

impl Folder {
    /// The innermost loop header of each block, and the headers, of the body
    /// as it came.
    fn loops(&mut self) {
        if self.inner.is_some() {
            return;
        }
        let loops = loopy::loops(&self.source.blocks, Some(self.entry));
        let mut inner: IndexMap<i64, i64> = IndexMap::default();
        // `loops` is innermost first.
        for found in &loops {
            for at in &found.body {
                inner.entry(*at).or_insert(found.header);
            }
        }
        self.headers = loops.iter().map(|found| found.header).collect();
        self.cyclic = cyclic_blocks(&self.source.blocks);
        self.inner = Some(inner);
    }

    /// How often `at` ran in the body as it came.
    fn runs_of(
        &mut self,
        at: i64,
    ) -> f64 {
        let at = self.origin.get(&at).copied().unwrap_or(at);
        let source = &self.source;
        self.frequency.get_or_insert_with(|| Frequency::of(source)).block(at)
    }

    /// The distinct predecessors of each block, in layout order.
    fn predecessors(&self) -> IndexMap<i64, Vec<i64>> {
        #[cfg(test)]
        PREDECESSOR_MAPS.with(|one| one.set(one.get() + 1));
        let mut preds: IndexMap<i64, Vec<i64>> = IndexMap::default();
        for block in &self.order {
            for (slot, to) in block.succ.iter().enumerate() {
                if !block.succ[..slot].contains(to) {
                    preds.entry(*to).or_default().push(block.at);
                }
            }
        }
        preds
    }

    fn reindex(&mut self) {
        self.pos = self.order.iter().enumerate().map(|(index, block)| (block.at, index)).collect();
    }

    fn block(
        &self,
        at: i64,
    ) -> &LirBlock {
        &self.order[self.pos[&at]]
    }

    fn successors(
        &self,
        at: i64,
    ) -> usize {
        self.block(at).succ.iter().collect::<HashSet<_>>().len()
    }

    /// The block after `at` in the layout that has any code: where control
    /// falls to.
    fn after(
        &self,
        at: i64,
    ) -> Option<i64> {
        self.order[self.pos[&at] + 1..]
            .iter()
            .find(|next| !next.phis.is_empty() || next.insns.iter().any(|one| !one.is_meta()))
            .map(|next| next.at)
    }

    /// The block before `at` in the layout that has any code.
    fn before(
        &self,
        at: i64,
    ) -> Option<i64> {
        self.order[..self.pos[&at]]
            .iter()
            .rev()
            .find(|previous| !previous.phis.is_empty() || previous.insns.iter().any(|one| !one.is_meta()))
            .map(|previous| previous.at)
    }

    /// `canFallThrough`: a candidate with its jump taken off can when the
    /// block laid out next is one of its successors (`getFallThrough`); any
    /// other block when its jump is to the block laid out next.
    fn falls(
        &self,
        at: i64,
    ) -> bool {
        if self.stripped.contains(&at) {
            return self.after(at).is_some_and(|next| self.block(at).succ.contains(&next));
        }
        let last = self.block(at).insns.iter().rev().find(|one| !one.is_meta());
        match last.and_then(|one| one.what.as_ref()) {
            Some(what) if what.op == Operation::Jump && !what.indirect && what.table.is_empty() => {
                what.target.is_some() && what.target == self.after(at)
            }
            Some(what) => !matches!(what.op, Operation::Return | Operation::Branch) && !what.indirect,
            None => true,
        }
    }

    fn ends_in_unreachable(
        &self,
        at: i64,
    ) -> bool {
        let block = self.block(at);
        if !block.succ.is_empty() {
            return false;
        }
        let last = block.insns.iter().rev().find(|one| !one.is_meta());
        last.and_then(|one| one.what.as_ref()).is_none_or(|what| !(what.op == Operation::Return || what.indirect))
    }

    /// `ProfitableToMerge`, after placement.
    fn profitable(
        &self,
        one: &Pot,
        other: &Pot,
        onward: Option<i64>,
        pred: Option<i64>,
    ) -> Option<usize> {
        self.profitable_because(one, other, onward, pred).0
    }

    /// `profitable`, and which of `ProfitableToMerge`'s rules said so.
    fn profitable_because(
        &self,
        one: &Pot,
        other: &Pot,
        onward: Option<i64>,
        pred: Option<i64>,
    ) -> (Option<usize>, &'static str) {
        let length = common(&one.seq, &other.seq, &self.calls);
        if length == 0 {
            return (None, "none");
        }
        let (full_one, full_other) = (length == one.seq.items.len(), length == other.seq.items.len());
        if (Some(one.at) == pred || Some(other.at) == pred) && self.successors(one.at) == 1 {
            let terms = if Some(one.at) == pred { other.seq.terms } else { one.seq.terms };
            if length > terms {
                return (Some(length), "pred");
            }
        }
        if full_one && full_other && self.ends_in_unreachable(one.at) && self.ends_in_unreachable(other.at) {
            return (Some(length), "unreachable");
        }
        if self.after(one.at) == Some(other.at) && full_other {
            return (Some(length), "layout-successor");
        }
        if self.after(other.at) == Some(one.at) && full_one {
            return (Some(length), "layout-successor");
        }
        if full_one && full_other {
            let both = |at: i64| {
                if !self.block(at).succ.is_empty() && !self.falls(at) {
                    return false;
                }
                at != self.entry && self.before(at).is_some_and(|previous| self.falls(previous))
            };
            if !both(one.at) || !both(other.at) {
                return (Some(length), "not-both-fall");
            }
        }
        let mut effective = length;
        if onward.is_some() && Some(one.at) != pred && Some(other.at) != pred && self.successors(one.at) == 1 {
            effective += 1;
        }
        if effective >= self.minimum {
            return (Some(length), "length");
        }
        if effective >= 2 && self.size && (full_one || full_other) {
            return (Some(length), "size");
        }
        (None, "short")
    }

    /// `ComputeSameTails`: the blocks of the last hash that share the longest
    /// tail with another, and how long it is.
    fn same_tails(
        &self,
        pots: &[Pot],
        hash: u64,
        onward: Option<i64>,
        pred: Option<i64>,
    ) -> (usize, Vec<Same>) {
        let mut longest = 0;
        let mut same: Vec<Same> = Vec::new();
        let mut highest = pots.len() - 1;
        let mut cur = pots.len() - 1;
        while cur != 0 && pots[cur].hash == hash {
            let mut at = cur - 1;
            while pots[at].hash == hash {
                if let Some(length) = self.profitable(&pots[cur], &pots[at], onward, pred) {
                    if length > longest {
                        same.clear();
                        longest = length;
                        highest = cur;
                        same.push(Same { pot: cur, start: pots[cur].seq.items.len() - length, at: pots[cur].at });
                    }
                    if highest == cur && length == longest {
                        same.push(Same { pot: at, start: pots[at].seq.items.len() - length, at: pots[at].at });
                    }
                }
                if at == 0 {
                    break;
                }
                at -= 1;
            }
            cur -= 1;
        }
        (longest, same)
    }

    /// `EstimateRuntime` of the block's items before `start`.
    fn estimate(
        pot: &Pot,
        start: usize,
    ) -> usize {
        pot.seq.items[..start]
            .iter()
            .map(|item| {
                let what = item.insn.what.as_ref();
                if what.is_some_and(|what| what.op == Operation::Call) {
                    10
                } else if what.is_some_and(|what| {
                    what.dests.iter().chain(&what.sources).any(|place| matches!(place, crate::model::ir::Loc::Mem(_)))
                }) {
                    2
                } else {
                    1
                }
            })
            .sum()
    }

    fn set(
        &mut self,
        block: LirBlock,
    ) {
        self.touch(block.at);
        let index = self.pos[&block.at];
        self.order[index] = block.into();
    }

    /// Remember the successors `at` has now, once, for `refresh`.
    fn touch(
        &mut self,
        at: i64,
    ) {
        if !self.touched.contains_key(&at) {
            let old = self.block(at).succ.iter().copied().collect();
            self.touched.insert(at, Some(old));
        }
    }

    /// The predecessor map `preds` brought up to date with the blocks changed
    /// since: what the pin has for free (a block keeps its predecessor list).
    fn refresh(
        &mut self,
        preds: &mut IndexMap<i64, Vec<i64>>,
    ) {
        for (at, old) in std::mem::take(&mut self.touched) {
            for to in old.into_iter().flatten() {
                if let Some(list) = preds.get_mut(&to) {
                    list.retain(|pred| *pred != at);
                }
            }
            for to in self.block(at).succ.iter() {
                let list = preds.entry(*to).or_default();
                if !list.contains(&at) {
                    list.push(at);
                }
            }
        }
    }

    /// `SplitMBBAt`: the block `at` up to `cut`, then a jump to a new block of
    /// the rest, laid out next. The new block's address.
    fn split(
        &mut self,
        at: i64,
        cut: usize,
    ) -> i64 {
        let block = self.block(at).clone();
        let made = self.next;
        self.next += 1;
        let tail = LirBlock {
            at: made,
            insns: block.insns[cut..].to_vec().into(),
            succ: _leaving(&block.succ, &block.insns[cut..], None).into(),
            phis: Default::default(),
            cold: block.cold,
            align: 0,
            max_skip: 0,
        };
        let last = block.insns[cut..].last().map_or(block.at, |one| one.at);
        let head = LirBlock {
            insns: block.insns[..cut].iter().cloned().chain([jump_to(last, made)]).collect::<Vec<_>>().into(),
            succ: _leaving(&block.succ, &block.insns[..cut], Some(made)).into(),
            ..block.clone()
        };
        self.touch(at);
        self.touched.insert(made, None);
        let index = self.pos[&at];
        self.order[index] = head.into();
        self.order.insert(index + 1, tail.into());
        self.reindex();
        if let Some(inner) = &mut self.inner {
            if let Some(header) = inner.get(&at).copied() {
                inner.insert(made, header);
            }
        }
        let from = self.origin.get(&at).copied().unwrap_or(at);
        self.origin.insert(made, from);
        for to in &block.succ {
            if let Some(&stated) = self.odds.taken.get(&(at, *to)) {
                self.odds.taken.insert((made, *to), stated);
            }
        }
        made
    }

    /// `TryTailMergeBlocks`.
    fn try_merge(
        &mut self,
        mut pots: Vec<Pot>,
        onward: Option<i64>,
        mut pred: Option<i64>,
    ) {
        pots.sort_by_key(|pot| (pot.hash, pot.at));
        while pots.len() > 1 {
            let hash = pots.last().expect("two or more").hash;
            let (_longest, mut same) = self.same_tails(&pots, hash, onward, pred);
            if same.is_empty() {
                while pots.last().is_some_and(|pot| pot.hash == hash) {
                    pots.pop();
                }
                continue;
            }
            let whole =
                |same: &Same, pots: &[Pot]| same.start == 0 || pots[same.pot].seq.items[..same.start].is_empty();
            let layout_successor = |one: &Same, other: &Same| self.pos[&other.at] == self.pos[&one.at] + 1;
            let mut keep = same.len();
            if same.len() == 2 && layout_successor(&same[0], &same[1]) && whole(&same[1], &pots) {
                keep = 1;
            } else if same.len() == 2 && layout_successor(&same[1], &same[0]) && whole(&same[0], &pots) {
                keep = 0;
            } else {
                for (index, one) in same.iter().enumerate() {
                    if one.at == self.entry && whole(one, &pots) {
                        continue;
                    }
                    if Some(one.at) == pred {
                        keep = index;
                        break;
                    }
                    if whole(one, &pots) {
                        keep = index;
                    }
                }
            }
            if keep == same.len() || (Some(same[keep].at) == pred && !whole(&same[keep], &pots)) {
                // None is only the tail: split one so that one is.
                let mut pick = 0;
                let mut time = usize::MAX;
                for (index, one) in same.iter().enumerate() {
                    if Some(one.at) == pred {
                        pick = index;
                        break;
                    }
                    let estimate = Self::estimate(&pots[one.pot], one.start);
                    if estimate <= time {
                        time = estimate;
                        pick = index;
                    }
                }
                keep = pick;
                let cut = pots[same[keep].pot].seq.items[same[keep].start].index;
                let from = same[keep].at;
                let made = self.split(from, cut);
                if pred == Some(from) {
                    pred = Some(made);
                }
                // The candidate is now the block of the tail alone.
                let pot = &mut pots[same[keep].pot];
                pot.at = made;
                pot.seq.items.drain(..same[keep].start);
                for item in &mut pot.seq.items {
                    item.index -= cut;
                }
                same[keep].at = made;
                same[keep].start = 0;
            }
            self.merge(&pots, &same, keep);
            let mut gone: Vec<usize> =
                same.iter().enumerate().filter(|(index, _)| *index != keep).map(|(_, one)| one.pot).collect();
            gone.sort_unstable_by(|a, b| b.cmp(a));
            for index in gone {
                pots.remove(index);
            }
            self.changed = true;
        }
    }

    /// `setCommonTailEdgeWeights`, `mergeCommonTails` and
    /// `replaceTailWithBranchTo`: the others end in a jump to the kept tail.
    fn merge(
        &mut self,
        pots: &[Pot],
        same: &[Same],
        keep: usize,
    ) {
        let tail = same[keep].at;
        let tail_succ: Vec<i64> = self.block(tail).succ.iter().copied().collect();
        // The kept tail goes each way as often as all of them did.
        if tail_succ.len() > 1 {
            let mut edges = vec![0.0; tail_succ.len()];
            for one in same {
                let runs = self.runs_of(one.at);
                let block = self.block(one.at);
                let (from, succ) = (block.at, block.succ.clone());
                for (slot, to) in tail_succ.iter().enumerate() {
                    edges[slot] += runs * self.odds.chance(from, &succ, *to);
                }
            }
            let sum: f64 = edges.iter().sum();
            if sum > 0.0 {
                for (to, edge) in tail_succ.iter().zip(&edges) {
                    let fixed = (edge / sum * BlockOdds::CERTAIN).round().min(f64::from(u32::MAX)) as u32;
                    self.odds.taken.insert((tail, *to), fixed);
                }
            }
        }
        if !self.origin.contains_key(&tail) {
            self.kept.push(tail);
        }
        // Their values are the kept tail's.
        let kept = &pots[same[keep].pot].seq.items[same[keep].start..];
        // The line of each kept instruction: what it and its copies share
        // (BranchFolding.cpp:933).
        let mut lines: IndexMap<usize, Option<crate::model::lir::SrcLoc>> =
            kept.iter().map(|item| (item.index, item.insn.line)).collect();
        for (index, one) in same.iter().enumerate() {
            if index == keep {
                continue;
            }
            let seq = &pots[one.pot].seq;
            let block = self.block(one.at).clone();
            let mut cut = seq.items[one.start].index;
            // Anchors just before the tail go with it: left, they would keep
            // the block that is now only a jump from being threaded around.
            while cut > 0 && super::_inert(&block.insns[cut - 1]) && block.insns[cut - 1].owned().is_empty() {
                cut -= 1;
            }
            let mut pairs: Vec<(u32, u32)> = Vec::new();
            for dropped in block.insns[cut..].iter().filter(|dropped| super::_inert(dropped)) {
                pairs.extend(dropped.defines.iter().zip(&dropped.uses).map(|(from, to)| (*from, *to)));
            }
            for (item, wanted) in seq.items[one.start..].iter().zip(kept) {
                let line = lines.get_mut(&wanted.index).expect("a kept item");
                *line = crate::model::lir::merged_line(*line, item.insn.line);
                pairs.extend(
                    item.insn
                        .defines
                        .iter()
                        .zip(&wanted.insn.defines)
                        .chain(item.insn.uses.iter().zip(&wanted.insn.uses))
                        .map(|(from, to)| (*from, *to)),
                );
            }
            for (from, to) in pairs.into_iter().filter(|(from, to)| from != to) {
                let (from, to) = (super::_root(&self.rename, from), super::_root(&self.rename, to));
                if from != to {
                    self.rename.insert(from, to);
                }
            }
            let last = block.insns[cut..].last().map_or(block.at, |one| one.at);
            let ended = LirBlock {
                insns: block.insns[..cut].iter().cloned().chain([jump_to(last, tail)]).collect::<Vec<_>>().into(),
                succ: _leaving(&block.succ, &block.insns[..cut], Some(tail)).into(),
                ..block.clone()
            };
            self.set(ended);
        }
        let block = self.block(tail).clone();
        let insns: Vec<Arc<Insn>> = block
            .insns
            .iter()
            .enumerate()
            .map(|(at, insn)| match lines.get(&at) {
                Some(line) if *line != insn.line => Arc::new(Insn { line: *line, ..(**insn).clone() }),
                _ => Arc::clone(insn),
            })
            .collect();
        self.set(block.with_insns(insns).into_inner());
    }

    /// The candidates whose hash another has, with the whole of their blocks
    /// seen: `ComputeSameTails` compares those alone, and reads their
    /// instructions from the end only as far as they match.
    fn completed(
        &self,
        mut pots: Vec<Pot>,
        onward: Option<i64>,
    ) -> Vec<Pot> {
        let mut count: HashMap<u64, usize> = HashMap::default();
        for pot in &pots {
            *count.entry(pot.hash).or_default() += 1;
        }
        pots.retain(|pot| count[&pot.hash] > 1);
        for pot in &mut pots {
            pot.seq = seq_of(self.block(pot.at), onward).expect("seen from its end above");
        }
        pots
    }

    /// `TailMergeBlocks`.
    fn run(&mut self) {
        // The blocks with no successor.
        let mut pots = Vec::new();
        for block in &self.order {
            if pots.len() == THRESHOLD {
                break;
            }
            if !self.tried.contains(&block.at)
                && block.succ.is_empty()
                && let Some(seq) = seq_from(block, end_of(block), None)
                && let Some(last) = seq.items.last()
                && !last.fixed
            {
                pots.push(Pot { hash: hash_of(&last.insn), at: block.at, seq });
            }
        }
        if pots.len() == THRESHOLD {
            self.tried.extend(pots.iter().map(|pot| pot.at));
        }
        let pots = self.completed(pots, None);
        if pots.len() >= 2 {
            self.try_merge(pots, None, None);
        }
        self.touched.clear();
        let mut preds = self.predecessors();
        let mut index = 1;
        while index < self.order.len() {
            let ibb = self.order[index].at;
            let mut unique: Vec<i64> = preds.get(&ibb).cloned().unwrap_or_default();
            if unique.len() < 2 {
                index += 1;
                continue;
            }
            let mut pots = Vec::new();
            unique.retain(|at| self.pos.contains_key(at));
            // Layout order, as the map made afresh has it.
            unique.sort_by_key(|at| self.pos[at]);
            for at in unique {
                if pots.len() == THRESHOLD {
                    break;
                }
                if self.tried.contains(&at) || at == ibb {
                    continue;
                }
                let block = self.block(at);
                if !block.phis.is_empty() {
                    continue;
                }
                if let Some(seq) = seq_from(block, end_of(block), Some(ibb))
                    && let Some(last) = seq.items.last()
                    && !last.fixed
                {
                    pots.push(Pot { hash: hash_of(&last.insn), at, seq });
                }
            }
            // Only blocks whose last instructions hash alike can share a tail.
            let mut seen: HashSet<u64> = HashSet::default();
            if !pots.iter().any(|pot| !seen.insert(pot.hash)) {
                index += 1;
                continue;
            }
            // Merging predecessors that are in the loop of the block would
            // make its header's neighbour the common tail; those of another
            // loop change that loop.
            self.loops();
            if self.headers.contains(&ibb) {
                index += 1;
                continue;
            }
            let inner = self.inner.as_ref().expect("the loops were found above");
            let loop_of = inner.get(&ibb).copied();
            pots.retain(|pot| inner.get(&pot.at).copied() == loop_of);
            // D1: a block on a cycle that ends in a branch is kept whole for
            // speed: dropped, its branch moves behind a jump it pays every
            // trip.
            if !self.size {
                pots.retain(|pot| pot.seq.terms == 0 || !self.cyclic.contains(&pot.at));
            }
            if pots.len() == THRESHOLD {
                self.tried.extend(pots.iter().map(|pot| pot.at));
            }
            let pots = self.completed(pots, Some(ibb));
            if pots.len() >= 2 {
                let before = self.order.len();
                self.stripped = pots.iter().map(|pot| pot.at).collect();
                let pred = self.before(ibb);
                self.try_merge(pots, Some(ibb), pred);
                self.stripped.clear();
                if self.order.len() != before {
                    // Blocks were split: the places after this one moved.
                    index = self.pos[&ibb];
                    self.refresh(&mut preds);
                }
            }
            index += 1;
        }
    }
}

/// The successors of a block cut in two that a part still names: the ways out
/// of `part` (a branch before its last jump has one), then `onward`, which the
/// first part jumps to.
fn _leaving(
    succ: &[i64],
    part: &[Arc<Insn>],
    onward: Option<i64>,
) -> Vec<i64> {
    let named: HashSet<i64> = part
        .iter()
        .filter_map(|one| one.what.as_ref())
        .filter(|what| matches!(what.op, Operation::Branch | Operation::Jump))
        .flat_map(|what| what.target.into_iter().chain(what.table.iter().copied()))
        .collect();
    let mut out: Vec<i64> = Vec::new();
    for one in succ.iter().copied().filter(|one| named.contains(one)).chain(onward) {
        if !out.contains(&one) {
            out.push(one);
        }
    }
    out
}

/// `TailMergeBlocks` to a fixed point on a placed body whose every fall-through
/// is an explicit jump (`None` where nothing merged); `size` for a body that is
/// optimised for size.
pub fn merged(
    body: &LirBody,
    size: bool,
    calls: Option<&IndexMap<i64, String>>,
) -> Result<Option<LirBody>, masm::Unprintable> {
    // A body presented before `_placed` has jumps that are fall-throughs.
    for block in &body.blocks {
        if masm::_falls_to(block, &body.name)?.is_some() {
            return Ok(None);
        }
    }
    // `MF.size() > 3`.
    if body.blocks.len() <= 3 {
        return Ok(None);
    }
    let mut folder = Folder {
        calls: calls.cloned().unwrap_or_default(),
        order: body.blocks.clone(),
        pos: IndexMap::default(),
        odds: body.odds.clone(),
        frequency: None,
        origin: IndexMap::default(),
        kept: Vec::new(),
        inner: None,
        headers: HashSet::default(),
        cyclic: HashSet::default(),
        source: body.clone(),
        tried: HashSet::default(),
        touched: IndexMap::default(),
        rename: IndexMap::default(),
        next: body.blocks.iter().map(|block| block.at).max().unwrap_or(0) + 1,
        minimum: if size { 2 } else { 3 },
        size,
        entry: body.entry,
        stripped: HashSet::default(),
        changed: false,
    };
    folder.reindex();
    let mut any = false;
    // `OptimizeFunction`'s loop, without the branch optimisation between.
    for _round in 0..body.blocks.len() + body.insns().len() {
        folder.changed = false;
        folder.run();
        if !folder.changed {
            break;
        }
        any = true;
    }
    if !any {
        return Ok(None);
    }
    let mut order = folder.order;
    if !folder.rename.is_empty() {
        let rename: IndexMap<u32, u32> =
            folder.rename.keys().map(|one| (*one, super::_root(&folder.rename, *one))).collect();
        order = order
            .iter()
            .map(|block| {
                block.with_insns(
                    block.insns.iter().map(|one| crate::backend::bodyfacts::_renamed(one, &rename)).collect(),
                )
            })
            .collect();
    }
    let mut out = body.with_blocks(order);
    out.odds = folder.odds;
    // A tail kept whole runs as often as everything that now reaches it: the
    // table forgets it, and a block the table does not hold is worked out from
    // its predecessors.
    if let Some(stated) = &body.frequencies {
        let mut table = stated.0.clone();
        for at in &folder.kept {
            table.shift_remove(at);
        }
        out.frequencies = Some(Arc::new(crate::model::lir::BlockFrequencies(table)));
        super::_forget_moved_inflow(body, &mut out);
    }
    Ok(Some(out))
}
