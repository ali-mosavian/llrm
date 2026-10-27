//! Which blocks are a loop, and how
//! deeply nested each one is.
//!
//! Python's frozensets become `BTreeSet`s and its result dicts `BTreeMap`s:
//! every caller only looks them up.  `irreducible` keeps block order where
//! its DFS start order depends on it.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

pub use llrm_mir::domtree_address::AddressDominance as Dominance;
pub use llrm_mir::forest::{Body, Forest, TreeLoop};
pub use llrm_support::graph::{Node, Predecessors, predecessors};

use crate::model::lir::LirBlock;
use crate::support::hash::IndexMap;

impl Node for LirBlock {
    fn at(&self) -> i64 {
        self.at
    }

    fn succ(&self) -> &[i64] {
        &self.succ
    }
}

impl Node for crate::model::lir::Block {
    fn at(&self) -> i64 {
        self.at
    }
    fn succ(&self) -> &[i64] {
        &self.succ
    }
}

/// Every block that must have run before each one, to a fixed point.
///
/// A block unreachable from the entry gets the empty set rather than "every
/// block". The sets are the square of a chain of blocks: the tests' and the
/// oracles' way to say what the tree says.
#[cfg(test)]
pub fn dominators<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> BTreeMap<i64, BTreeSet<i64>> {
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
    forest: Option<Rc<Forest>>,
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
            && one
                .shape
                .iter()
                .zip(blocks)
                .all(|((at, succ), block)| *at == block.at() && succ.as_slice() == block.succ())
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
            shapes.insert(
                0,
                Shaped {
                    entry,
                    shape: blocks.iter().map(|block| (block.at(), block.succ().to_vec())).collect(),
                    dominance: None,
                    loops: None,
                    forest: None,
                },
            );
            shapes.truncate(REMEMBERED);
        }
        write(&mut shapes[0], computed.clone());
    });
    computed
}

pub fn dominance<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> Rc<Dominance> {
    shaped(
        blocks,
        entry,
        |one| one.dominance.clone(),
        |one, found| one.dominance = Some(found),
        || Rc::new(Dominance::of(blocks.iter().map(|block| (block.at(), block.succ())), entry)),
    )
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
    pub body: Body,
}

/// (latch, header) for every edge to a block that dominates its source.
pub fn back_edges<N: Node>(
    blocks: &[N],
    dominance: &Dominance,
) -> Vec<(i64, i64)> {
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
pub fn _body(
    latch: i64,
    header: i64,
    preds: &Predecessors,
) -> BTreeSet<i64> {
    let mut body = BTreeSet::from([header]);
    if latch == header {
        return body;
    }
    body.insert(latch);
    let mut pending = vec![latch];
    while let Some(at) = pending.pop() {
        #[cfg(test)]
        FLOOD_VISITS.with(|visits| visits.set(visits.get() + 1));
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
/// theirs. The bodies are named for the caller who asks for them: a caller
/// that only asks where a block stands, or how deep, asks the [`Forest`].
pub fn loops<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> Vec<Loop> {
    llrm_mir::counts::bump(llrm_mir::counts::Event::LirLoops);
    let found = shaped(
        blocks,
        entry,
        |one| one.loops.clone(),
        |one, found| one.loops = Some(found),
        || {
            let named = named(&forest_of(blocks, entry));
            if llrm_support::env_flag!("LLRM_CHECK_LOOPS") {
                assert!(
                    flat(&named) == flooded(blocks, entry),
                    "LLRM_CHECK_LOOPS: the forest's loops differ from a flood"
                );
            }
            Rc::new(named)
        },
    );
    Vec::clone(&found)
}

/// The loops as `loops` gives them: bodies named, innermost first.
fn named(forest: &Rc<Forest>) -> Vec<Loop> {
    let mut found: Vec<Loop> = (0..forest.loops.len())
        .map(|index| Loop {
            header: forest.loops[index].header,
            latches: forest.loops[index].latches.iter().copied().collect(),
            body: Body::new(forest, index),
        })
        .collect();
    found.sort_by_key(|loop_| loop_.body.len());
    found
}

/// `forest`, found once for the shape of the blocks.
pub fn forest_of<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> Rc<Forest> {
    shaped(
        blocks,
        entry,
        |one| one.forest.clone(),
        |one, found| one.forest = Some(found),
        || {
            let doms = dominance(blocks, entry);
            let nodes: Vec<(i64, &[i64])> = blocks.iter().map(|block| (block.at(), block.succ())).collect();
            Rc::new(llrm_mir::forest::forest(&nodes, entry, &|at| doms.reachable(at), &|dominator, at| {
                doms.dominates(dominator, at)
            }))
        },
    )
}

/// The loops by a flood from each latch, for `LLRM_CHECK_LOOPS` to hold the
/// forest to.
fn flooded<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> Vec<(i64, BTreeSet<i64>, BTreeSet<i64>)> {
    let doms = dominance(blocks, entry);
    let preds = predecessors(&blocks.iter().filter(|block| doms.reachable(block.at())).collect::<Vec<_>>());

    let mut latches: IndexMap<i64, BTreeSet<i64>> = IndexMap::default();
    let mut bodies: IndexMap<i64, BTreeSet<i64>> = IndexMap::default();
    for (latch, header) in back_edges(blocks, &doms) {
        latches.entry(header).or_default().insert(latch);
        bodies.entry(header).or_default().extend(_body(latch, header, &preds));
    }

    let mut found =
        latches.iter().map(|(&header, latch)| (header, latch.clone(), bodies[&header].clone())).collect::<Vec<_>>();
    found.sort_by_key(|loop_| loop_.2.len());
    found
}

/// The loops as sets, to hold them to a flood.
pub(crate) fn flat(loops: &[Loop]) -> Vec<(i64, BTreeSet<i64>, BTreeSet<i64>)> {
    loops.iter().map(|one| (one.header, one.latches.clone(), one.body.to_set())).collect()
}

/// Blocks left in a cycle once every natural loop's back edge is cut.
///
/// Decided by actually cutting the edges and looking for a remaining cycle,
/// not by address order.
pub fn irreducible<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> BTreeSet<i64> {
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
pub fn depth<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> BTreeMap<i64, usize> {
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
pub fn immediate_dominators<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> BTreeMap<i64, Option<i64>> {
    // The tree `dominance` found already says it; naming every block's
    // dominators to pick the one with the most was the square of the blocks
    // on a chain of them.
    let tree = dominance(blocks, entry);
    let found = blocks.iter().map(|block| (block.at(), tree.immediate(block.at()))).collect();
    #[cfg(test)]
    assert_eq!(
        found,
        immediate_dominators_named(blocks, entry),
        "the tree's immediate dominators are not the named ones"
    );
    found
}

/// `immediate_dominators` from the sets of dominators, the oracle for the
/// tree's.
#[cfg(test)]
fn immediate_dominators_named<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> BTreeMap<i64, Option<i64>> {
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
/// a phi belongs in: for each block, the joins it reaches without dominating
/// them (Cytron et al., by the walk up the tree from each predecessor of a
/// join). A block the entry does not reach has none, and is no predecessor.
pub fn frontiers<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> BTreeMap<i64, BTreeSet<i64>> {
    let tree = dominance(blocks, entry);
    let live = blocks.iter().filter(|block| tree.reachable(block.at())).collect::<Vec<_>>();
    let preds = predecessors(&live);
    let mut found = blocks.iter().map(|block| (block.at(), BTreeSet::new())).collect::<BTreeMap<_, _>>();
    for block in &live {
        if preds[&block.at()].len() < 2 {
            continue;
        }
        let stop = tree.immediate(block.at());
        for &one in &preds[&block.at()] {
            let mut runner = Some(one);
            while let Some(at) = runner {
                if Some(at) == stop {
                    break;
                }
                found.get_mut(&at).expect("live").insert(block.at());
                runner = tree.immediate(at);
            }
        }
    }
    found
}

// Blocks a flood from each latch took up, for a test of how they grow with the
// nesting.
#[cfg(test)]
thread_local! {
    static FLOOD_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
#[path = "loops_tests.rs"]
mod tests;
