//! Which blocks are a loop, and how deeply nested each one is: copied from
//! llrm-core's `analysis/loops.rs`, the port of `qbopt/analysis/loops.py`,
//! for any IR whose blocks are `Node`s.
//!
//! Python's frozensets become `BTreeSet`s and its result dicts `BTreeMap`s:
//! every caller only looks them up.  `irreducible` keeps block order where
//! its DFS start order depends on it.

use std::collections::{BTreeMap, BTreeSet};

pub use llrm_support::graph::{Dominance, Dominates, Node, dominance, dominators, predecessors, reverse_postorder};
use llrm_support::hash::IndexMap;

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
pub fn back_edges<N: Node>(
    blocks: &[N],
    dominance: &impl Dominates,
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
    preds: &BTreeMap<i64, BTreeSet<i64>>,
) -> BTreeSet<i64> {
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
pub fn loops<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> Vec<Loop> {
    let doms = dominance(blocks, entry);
    let preds = predecessors(&blocks.iter().filter(|block| doms.reachable(block.at())).collect::<Vec<_>>());

    let mut latches: IndexMap<i64, BTreeSet<i64>> = IndexMap::default();
    let mut bodies: IndexMap<i64, BTreeSet<i64>> = IndexMap::default();
    for (latch, header) in back_edges(blocks, &*doms) {
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
pub fn irreducible<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> BTreeSet<i64> {
    irreducible_under(blocks, &*dominance(blocks, entry))
}

/// `irreducible`, of dominance found already.
pub fn irreducible_under<N: Node>(
    blocks: &[N],
    doms: &impl Dominates,
) -> BTreeSet<i64> {
    let known = blocks.iter().filter(|block| doms.reachable(block.at())).map(Node::at).collect::<BTreeSet<_>>();
    let cut = back_edges(blocks, doms).into_iter().collect::<BTreeSet<_>>();
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
pub fn frontiers<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> BTreeMap<i64, BTreeSet<i64>> {
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
mod loops_tests;
