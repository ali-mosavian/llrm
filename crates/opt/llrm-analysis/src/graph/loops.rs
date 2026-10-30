//! Which blocks are a loop, and how deeply nested each one is, for any IR whose
//! blocks are `Node`s.
//!
//! # How it works
//! Dominance comes from `llrm_support::graph` (re-exported here). A back edge
//! is an edge whose target dominates its source. Back edges that share a
//! header are one loop: its `latches` are their sources, and its `body` is the
//! union of the blocks that reach each latch without passing back through the
//! header (`_body` puts the header in first, which stops the walk).
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   b0["<b>b0</b>"]:::mid
//!   b1["<b>b1</b><br/><small>loop header</small>"]:::mid
//!   b2["<b>b2</b><br/><small>loop header, self loop</small>"]:::mid
//!   b3["<b>b3</b>"]:::mid
//!   b4["<b>b4</b>"]:::mid
//!   b0 --> b1
//!   b1 --> b4
//!   b1 --> b2
//!   b2 --> b2
//!   b2 --> b3
//!   b3 --> b1
//! ```
//!
//! *Back edges b3->b1 and b2->b2. Loop h=b1 has latches {b3} and body
//! {b1,b2,b3}; loop h=b2 has latches {b2} and body {b2}*
//!
//! `loops` sorts by body size, so an inner loop precedes the loops around it.
//! `depth` counts the loops each block is in, 0 for straight-line code.
//! `immediate_dominators` takes, of a block's strict dominators, the one with
//! the most dominators itself. `frontiers` is the dominance frontier: the
//! blocks a phi belongs in, found by walking each join's predecessors up to the
//! join's immediate dominator.
//!
//! `irreducible` finds blocks left in a cycle once every back edge is cut. It
//! cuts them and runs a three-colour DFS on the rest: an edge into a grey block
//! closes a cycle no natural loop explains, and its target is reported.
//! Unreachable blocks are ignored. `irreducible_under` takes dominance already
//! found.
//!
//! # Example
//! A graph given as `block(at, succ)` pairs, with a loop nested in a loop:
//!
//! ```text
//! 0 -> 1
//! 1 -> 2
//! 2 -> 3
//! 3 -> 2, 4
//! 4 -> 1, 5
//! 5 ->
//! ```
//!
//! | Query | Result |
//! | --- | --- |
//! | `loops` headers, in order | 2, 1 |
//! | body of loop 2 | {2, 3} |
//! | body of loop 1 | {1, 2, 3, 4} |
//! | `depth` of blocks 0..5 | 0, 1, 2, 2, 1, 0 |
//! | `irreducible` | none |
//!
//! From unit test
//! `nested_loops_come_innermost_first_and_the_outer_body_holds_the_inner` in
//! `loops_tests.rs`.
//!
//! # Provides
//! `Loop`, `back_edges`, `loops`, `depth`, `irreducible`, `irreducible_under`,
//! `immediate_dominators` and `frontiers`. Consumers: [`crate::cfg`]
//! (`Dominance::irreducible`), [`crate::noreturn`], and the loop passes of
//! `llrm-transforms` (grep `graph::loops`).
//!
//! # See also
//! LLVM `LoopInfo`.

use std::collections::{BTreeMap, BTreeSet};

pub use llrm_mir::domtree_address::{AddressDominance as Dominance, Dominates};
pub use llrm_mir::forest::Body;
pub use llrm_support::graph::{Node, predecessors, reverse_postorder};
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
    pub body: Body,
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

/// Every natural loop, innermost first where they nest.
///
/// Back edges sharing a header are one loop whose body is the union of
/// theirs.
pub fn loops<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> Vec<Loop> {
    let doms = dominance(blocks, entry);
    let nodes: Vec<(i64, &[i64])> = blocks.iter().map(|block| (block.at(), block.succ())).collect();
    let forest =
        std::rc::Rc::new(llrm_mir::forest::forest(&nodes, entry, &|at| doms.reachable(at), &|dominator, at| {
            doms.dominates(dominator, at)
        }));
    let mut found = (0..forest.loops.len())
        .map(|index| Loop {
            header: forest.loops[index].header,
            latches: forest.loops[index].latches.iter().copied().collect(),
            body: Body::new(&forest, index),
        })
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
    irreducible_under(blocks, &dominance(blocks, entry))
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

/// The dominance of `blocks` from `entry` (the first block by default).
pub fn dominance<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> Dominance {
    Dominance::of(blocks.iter().map(|block| (block.at(), block.succ())), entry)
}

/// Every block that must have run before each one. A block unreachable from
/// the entry gets the empty set rather than "every block".
pub fn dominators<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> BTreeMap<i64, BTreeSet<i64>> {
    dominance(blocks, entry).named()
}

/// Each block's nearest strict dominator, or None for the entry and for
/// anything unreachable.
pub fn immediate_dominators<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> BTreeMap<i64, Option<i64>> {
    let tree = dominance(blocks, entry);
    blocks.iter().map(|block| (block.at(), tree.immediate(block.at()))).collect()
}

/// Where a definition stops being the only one that reaches -- the blocks
/// a phi belongs in.
pub fn frontiers<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> BTreeMap<i64, BTreeSet<i64>> {
    let tree = dominance(blocks, entry);
    let live = blocks.iter().filter(|block| tree.reachable(block.at())).collect::<Vec<_>>();
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
