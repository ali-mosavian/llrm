//! Port of `qbopt/analysis/loops.py`: which blocks are a loop, and how
//! deeply nested each one is.
//!
//! Python's frozensets become `BTreeSet`s and its result dicts `BTreeMap`s:
//! every caller only looks them up.  `irreducible` keeps block order where
//! its DFS start order depends on it.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use crate::support::bits::Bits;
use crate::support::hash::IndexMap;

use crate::model::lir::LirBlock;

/// All these walks read of a block: where it is and where it goes.
pub trait Node {
    fn at(&self) -> i64;
    fn succ(&self) -> &[i64];
}

impl Node for LirBlock {
    fn at(&self) -> i64 {
        self.at
    }

    fn succ(&self) -> &[i64] {
        &self.succ
    }
}

impl<T: Node> Node for &T {
    fn at(&self) -> i64 {
        (*self).at()
    }

    fn succ(&self) -> &[i64] {
        (*self).succ()
    }
}

pub fn predecessors<N: Node>(blocks: &[N]) -> BTreeMap<i64, BTreeSet<i64>> {
    let known = blocks.iter().map(Node::at).collect::<BTreeSet<_>>();
    let mut found = blocks.iter().map(|block| (block.at(), BTreeSet::new())).collect::<BTreeMap<_, _>>();
    for block in blocks {
        for successor in block.succ() {
            if known.contains(successor) {
                found.get_mut(successor).expect("known").insert(block.at());
            }
        }
    }
    found
}

/// Every block that must have run before each one, to a fixed point.
///
/// A block unreachable from the entry gets the empty set rather than "every
/// block".
pub fn dominators<N: Node>(blocks: &[N], entry: Option<i64>) -> BTreeMap<i64, BTreeSet<i64>> {
    dominance(blocks, entry).named()
}

/// What dominance and loops of one CFG shape came to: LLVM's `CFGAnalyses`,
/// which every pass that leaves the CFG alone preserves. Keyed by the shape
/// itself -- entry, and each block's address and successors in order -- so a
/// pass that changes only operations finds them, and no pass has to say so.
struct Shaped {
    entry: Option<i64>,
    shape: Vec<(i64, Vec<i64>)>,
    dominance: Option<Rc<Dominance>>,
    loops: Option<Rc<Vec<Loop>>>,
}

/// Shapes remembered: a transaction alternates between few of them.
const REMEMBERED: usize = 8;

thread_local! {
    static SHAPES: RefCell<Vec<Shaped>> = const { RefCell::new(Vec::new()) };
}

/// `read` of `blocks`' shape, from `compute` on first asking.
fn shaped<N: Node, T: Clone>(
    blocks: &[N],
    entry: Option<i64>,
    read: impl Fn(&Shaped) -> Option<T>,
    write: impl FnOnce(&mut Shaped, T),
    compute: impl FnOnce() -> T,
) -> T {
    let same = |one: &Shaped| {
        one.entry == entry
            && one.shape.len() == blocks.len()
            && one.shape.iter().zip(blocks).all(|((at, succ), block)| *at == block.at() && succ.as_slice() == block.succ())
    };
    let found = SHAPES.with(|shapes| {
        let mut shapes = shapes.borrow_mut();
        let index = shapes.iter().position(same)?;
        let one = shapes.remove(index);
        let answer = read(&one);
        shapes.insert(0, one);
        answer
    });
    if let Some(found) = found {
        return found;
    }
    let computed = compute();
    SHAPES.with(|shapes| {
        let mut shapes = shapes.borrow_mut();
        if !shapes.first().is_some_and(same) {
            shapes.insert(0, Shaped {
                entry,
                shape: blocks.iter().map(|block| (block.at(), block.succ().to_vec())).collect(),
                dominance: None,
                loops: None,
            });
            shapes.truncate(REMEMBERED);
        }
        write(&mut shapes[0], computed.clone());
    });
    computed
}

/// The dominator tree over the sorted block addresses, with a preorder interval for each block: `a`
/// dominates `b` where `b` is in `a`'s interval. Naming every block's dominators as bits is quadratic.
pub struct Dominance {
    ats: Vec<i64>,
    /// The immediate dominator's slot; the entry's own, and unreachable blocks', is `NONE`.
    idom: Vec<u32>,
    /// Preorder numbers over the dominator tree; `NONE` for a block the entry does not reach.
    enter: Vec<u32>,
    /// The last preorder number inside the block's subtree.
    last: Vec<u32>,
}

const NONE: u32 = u32::MAX;

impl Dominance {
    fn slot(&self, at: i64) -> Option<usize> {
        self.ats.binary_search(&at).ok()
    }

    /// Whether the entry reaches `at`: only then does anything dominate it.
    pub fn reachable(&self, at: i64) -> bool {
        self.slot(at).is_some_and(|slot| self.enter[slot] != NONE)
    }

    pub fn dominates(&self, dominator: i64, at: i64) -> bool {
        match (self.slot(dominator), self.slot(at)) {
            (Some(dominator), Some(at)) => self.enter[at] != NONE && self.enter[dominator] <= self.enter[at] && self.enter[at] <= self.last[dominator],
            _ => false,
        }
    }

    fn named(&self) -> BTreeMap<i64, BTreeSet<i64>> {
        (0..self.ats.len())
            .map(|slot| {
                let mut above = BTreeSet::new();
                if self.enter[slot] != NONE {
                    let mut at = slot as u32;
                    loop {
                        above.insert(self.ats[at as usize]);
                        if self.idom[at as usize] == NONE {
                            break;
                        }
                        at = self.idom[at as usize];
                    }
                }
                (self.ats[slot], above)
            })
            .collect()
    }
}

pub fn dominance<N: Node>(blocks: &[N], entry: Option<i64>) -> Rc<Dominance> {
    shaped(
        blocks,
        entry,
        |one| one.dominance.clone(),
        |one, found| one.dominance = Some(found),
        || Rc::new(_dominance(blocks, entry)),
    )
}

/// Cooper, Harvey and Kennedy's iteration over reverse postorder, on the blocks the entry reaches.
fn _dominance<N: Node>(blocks: &[N], entry: Option<i64>) -> Dominance {
    let ats = blocks.iter().map(Node::at).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
    let count = ats.len();
    let mut found = Dominance { ats, idom: vec![NONE; count], enter: vec![NONE; count], last: vec![NONE; count] };
    if blocks.is_empty() {
        return found;
    }
    let start = entry.unwrap_or_else(|| blocks[0].at());
    let Some(start) = found.slot(start) else { return found };
    // Each address's successors, as slots; a later block of one address replaces an earlier one's.
    let mut successors: Vec<Vec<u32>> = vec![Vec::new(); count];
    for block in blocks {
        let from = found.slot(block.at()).expect("a block address");
        successors[from] = block.succ().iter().filter_map(|to| found.slot(*to)).map(|to| to as u32).collect();
    }

    // Postorder by an explicit stack.
    let mut order = Vec::new();
    let mut seen = vec![false; count];
    let mut stack = vec![(start, 0_usize)];
    seen[start] = true;
    while let Some(&(at, next)) = stack.last() {
        if let Some(&to) = successors[at].get(next) {
            stack.last_mut().expect("nonempty").1 += 1;
            if !seen[to as usize] {
                seen[to as usize] = true;
                stack.push((to as usize, 0));
            }
        } else {
            order.push(at);
            stack.pop();
        }
    }
    let mut number = vec![NONE; count];
    for (index, &at) in order.iter().enumerate() {
        number[at] = index as u32;
    }
    let mut predecessors: Vec<Vec<u32>> = vec![Vec::new(); count];
    for &at in &order {
        for &to in &successors[at] {
            predecessors[to as usize].push(at as u32);
        }
    }

    let mut idom = vec![NONE; count];
    idom[start] = start as u32;
    let mut changing = true;
    while changing {
        changing = false;
        for &at in order.iter().rev() {
            if at == start {
                continue;
            }
            let mut now = NONE;
            for &from in &predecessors[at] {
                if idom[from as usize] == NONE {
                    continue;
                }
                now = if now == NONE {
                    from
                } else {
                    let (mut one, mut other) = (now, from);
                    while one != other {
                        while number[one as usize] < number[other as usize] {
                            one = idom[one as usize];
                        }
                        while number[other as usize] < number[one as usize] {
                            other = idom[other as usize];
                        }
                    }
                    one
                };
            }
            if now != idom[at] {
                idom[at] = now;
                changing = true;
            }
        }
    }
    idom[start] = NONE;

    // Preorder intervals of the tree.
    let mut children: Vec<Vec<u32>> = vec![Vec::new(); count];
    for &at in order.iter().rev() {
        if idom[at] != NONE {
            children[idom[at] as usize].push(at as u32);
        }
    }
    let mut clock = 0;
    let mut walk = vec![(start, 0_usize)];
    found.enter[start] = 0;
    clock += 1;
    while let Some(&(at, next)) = walk.last() {
        if let Some(&child) = children[at].get(next) {
            walk.last_mut().expect("nonempty").1 += 1;
            found.enter[child as usize] = clock;
            clock += 1;
            walk.push((child as usize, 0));
        } else {
            found.last[at] = clock - 1;
            walk.pop();
        }
    }
    found.idom = idom;
    found
}

/// One natural loop: where control comes back to, and what is inside.
///
/// Keyed by header, not by back edge: several latches are one loop.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Loop {
    pub header: i64,
    /// every block whose own edge goes back to the header
    pub latches: BTreeSet<i64>,
    /// every block in the loop, the header included
    pub body: BTreeSet<i64>,
}

/// (latch, header) for every edge to a block that dominates its source.
pub fn back_edges<N: Node>(blocks: &[N], dominance: &Dominance) -> Vec<(i64, i64)> {
    let known = blocks.iter().map(Node::at).collect::<BTreeSet<_>>();
    let mut found = Vec::new();
    for block in blocks {
        for &successor in block.succ() {
            if known.contains(&successor) && dominance.dominates(successor, block.at()) {
                found.push((block.at(), successor));
            }
        }
    }
    found
}

/// Everything that reaches the latch without going back through the header.
///
/// The header goes in before the walk starts, which is what stops it.
pub fn _body(latch: i64, header: i64, preds: &BTreeMap<i64, BTreeSet<i64>>) -> BTreeSet<i64> {
    let mut body = BTreeSet::from([header]);
    if latch == header {
        return body;
    }
    body.insert(latch);
    let mut pending = vec![latch];
    while let Some(at) = pending.pop() {
        for &one in preds.get(&at).into_iter().flatten() {
            if !body.contains(&one) {
                body.insert(one);
                pending.push(one);
            }
        }
    }
    body
}

/// Every natural loop, innermost first where they nest.
///
/// Back edges sharing a header are one loop whose body is the union of
/// theirs.
pub fn loops<N: Node>(blocks: &[N], entry: Option<i64>) -> Vec<Loop> {
    let found = shaped(
        blocks,
        entry,
        |one| one.loops.clone(),
        |one, found| one.loops = Some(found),
        || Rc::new(_loops(blocks, entry)),
    );
    Vec::clone(&found)
}

fn _loops<N: Node>(blocks: &[N], entry: Option<i64>) -> Vec<Loop> {
    let doms = dominance(blocks, entry);
    let preds = predecessors(&blocks.iter().filter(|block| doms.reachable(block.at())).collect::<Vec<_>>());

    let mut latches: IndexMap<i64, BTreeSet<i64>> = IndexMap::default();
    let mut bodies: IndexMap<i64, BTreeSet<i64>> = IndexMap::default();
    for (latch, header) in back_edges(blocks, &doms) {
        latches.entry(header).or_default().insert(latch);
        bodies.entry(header).or_default().extend(_body(latch, header, &preds));
    }

    let mut found = latches
        .iter()
        .map(|(&header, latch)| Loop { header, latches: latch.clone(), body: bodies[&header].clone() })
        .collect::<Vec<_>>();
    found.sort_by_key(|loop_| loop_.body.len());
    found
}

/// Blocks left in a cycle once every natural loop's back edge is cut.
///
/// Decided by actually cutting the edges and looking for a remaining cycle,
/// not by address order.
pub fn irreducible<N: Node>(blocks: &[N], entry: Option<i64>) -> BTreeSet<i64> {
    let doms = dominance(blocks, entry);
    let known = blocks.iter().filter(|block| doms.reachable(block.at())).map(Node::at).collect::<BTreeSet<_>>();
    let cut = back_edges(blocks, &doms).into_iter().collect::<BTreeSet<_>>();
    let forward = blocks
        .iter()
        .filter(|block| known.contains(&block.at()))
        .map(|block| {
            let successors = block
                .succ()
                .iter()
                .copied()
                .filter(|s| known.contains(s) && !cut.contains(&(block.at(), *s)))
                .collect::<Vec<_>>();
            (block.at(), successors)
        })
        .collect::<IndexMap<_, _>>();

    // three-colour DFS: grey is the current stack, so an edge into it closes
    // a cycle that survived the cut
    const WHITE: u8 = 0;
    const GREY: u8 = 1;
    const BLACK: u8 = 2;
    let mut colour = forward.keys().map(|&at| (at, WHITE)).collect::<IndexMap<_, _>>();
    let mut found = BTreeSet::new();

    for &start in forward.keys() {
        if colour[&start] != WHITE {
            continue;
        }
        // (block, index of its next unvisited successor): Python's iterator
        let mut stack = vec![(start, 0_usize)];
        colour[&start] = GREY;
        while let Some(&(at, next)) = stack.last() {
            let successors = &forward[&at];
            let mut index = next;
            let mut descended = false;
            while index < successors.len() {
                let successor = successors[index];
                index += 1;
                if colour[&successor] == GREY {
                    found.insert(successor);
                } else if colour[&successor] == WHITE {
                    colour[&successor] = GREY;
                    stack.last_mut().expect("nonempty").1 = index;
                    stack.push((successor, 0));
                    descended = true;
                    break;
                }
            }
            if !descended {
                colour[&at] = BLACK;
                stack.pop();
            }
        }
    }
    found
}

/// How many loops each block is inside -- 0 for straight-line code.
pub fn depth<N: Node>(blocks: &[N], entry: Option<i64>) -> BTreeMap<i64, usize> {
    let mut found = blocks.iter().map(|block| (block.at(), 0_usize)).collect::<BTreeMap<_, _>>();
    for loop_ in loops(blocks, entry) {
        for at in &loop_.body {
            if let Some(count) = found.get_mut(at) {
                *count += 1;
            }
        }
    }
    found
}

/// Each block's nearest strict dominator, or None for the entry and for
/// anything unreachable.
///
/// The nearest strict dominator is the one with the most dominators of its
/// own.
pub fn immediate_dominators<N: Node>(blocks: &[N], entry: Option<i64>) -> BTreeMap<i64, Option<i64>> {
    let doms = dominators(blocks, entry);
    let mut found = BTreeMap::new();
    for block in blocks {
        let empty = BTreeSet::new();
        let strict = doms.get(&block.at()).unwrap_or(&empty).iter().copied().filter(|&one| one != block.at());
        // max() keeps the first of equal keys
        let nearest = strict.fold(None, |best: Option<i64>, one| match best {
            Some(previous) if doms[&one].len() <= doms[&previous].len() => Some(previous),
            _ => Some(one),
        });
        found.insert(block.at(), nearest);
    }
    found
}

/// Where a definition stops being the only one that reaches -- the blocks
/// a phi belongs in.
pub fn frontiers<N: Node>(blocks: &[N], entry: Option<i64>) -> BTreeMap<i64, BTreeSet<i64>> {
    let doms = dominators(blocks, entry);
    let live = blocks.iter().filter(|block| !doms[&block.at()].is_empty()).collect::<Vec<_>>();
    let idom = immediate_dominators(&live, entry);
    let preds = predecessors(&live);
    let mut found = blocks.iter().map(|block| (block.at(), BTreeSet::new())).collect::<BTreeMap<_, _>>();
    for block in &live {
        if preds[&block.at()].len() < 2 {
            continue;
        }
        for &one in &preds[&block.at()] {
            let mut runner = Some(one);
            while let Some(at) = runner {
                if Some(at) == idom[&block.at()] {
                    break;
                }
                found.get_mut(&at).expect("live").insert(block.at());
                runner = idom.get(&at).copied().flatten();
            }
        }
    }
    found
}

#[cfg(test)]
#[path = "loops_tests.rs"]
mod tests;
