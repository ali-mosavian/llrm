//! Iterated dominance frontiers: where an SSA value defined in a set of blocks
//! needs a phi (forward), or where the control dependences of a set of blocks
//! lie (reverse, on the post-dominator tree).
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `GenericIteratedDominanceFrontier.h`
//! (`IDFCalculatorBase::calculate`) and the `ForwardIDFCalculator` and
//! `ReverseIDFCalculator` of `Analysis/IteratedDominanceFrontier.h`. It is the
//! linear-time DJ-graph algorithm: the defining blocks are visited from the
//! bottom of the tree upwards; from each, the subtree below it is walked and
//! every edge that leaves the subtree to a block at the same level or above is
//! in the frontier, which then is a defining block in its turn. A live-in set
//! prunes the frontier to blocks where the value is live, so no dead phi is
//! asked for. One call is O(blocks + edges of the walked subtrees), each node
//! once.
//!
//! The blocks come out in the order LLVM's calculator gives them, which the
//! golden file `idf_golden.txt` (`tools/llvmport/idf_dump.sh`) checks.
//!
//! DEVIATE (anything MIR or Rust lacks):
//! 1. The defining and live-in sets are [`IdSet`]s, not `SmallPtrSet`s; their
//!    iteration order differs, which cannot change the result: the queue's key
//!    (level, DFS number) is unique per block.
//! 2. The graph is the [`DomGraph`] the tree was built over, not a
//!    `ChildrenGetter`; the `GraphDiff` form of `ChildrenGetterTy<BasicBlock>`
//!    is not translated (nothing asks for a view of a CFG with pending
//!    updates).
//! 3. `Function::predecessors` lists a block once and in MIR's use order (see
//!    `domtree` DEVIATE 7): the reverse frontier is the same set, its order can
//!    differ from LLVM's on a switch with repeated targets.
//!
//! # Used by
//! Nothing yet. Readers to migrate: `mem2reg` in llrm-transforms places its
//! phis with `Dominance::frontiers` iterated (`iterated_frontier`), which
//! yields the same set; ADCE's control-dependence marking (reverse) is the
//! next consumer.

use std::collections::BinaryHeap;

use crate::dense::IdSet;
use crate::domtree::{DomGraph, DomTree, DomTreeBase, PostDomTree};
use crate::module::BlockId;

/// `IDFCalculatorBase<NodeTy, IsPostDom>::calculate` with the defining blocks
/// `defining` and, if given, the blocks `live_in`: the iterated dominance
/// frontier of `defining` in `tree` (the post-dominance one if `POST`),
/// pruned to `live_in`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericIteratedDominanceFrontier.h#L131>
pub fn calculate<G: DomGraph, const POST: bool>(
    tree: &DomTreeBase<POST>,
    graph: &G,
    defining: &IdSet<BlockId>,
    live_in: Option<&IdSet<BlockId>>,
) -> Vec<BlockId> {
    // Handled from the bottom of the tree upwards; the DFS number makes the
    // order deterministic.
    let mut queue: BinaryHeap<((u32, u32), BlockId)> = BinaryHeap::new();
    tree.update_dfs_numbers();
    let key = |block: BlockId| {
        let level = tree.level(block).expect("a block of the tree");
        (level, tree.span(block).expect("a block of the tree").0)
    };
    let mut visited_queue = IdSet::<BlockId>::new();
    let mut visited_worklist = IdSet::<BlockId>::new();
    for block in defining.iter() {
        if tree.contains(block) {
            queue.push((key(block), block));
            visited_worklist.insert(block);
        }
    }
    let mut frontier = Vec::new();
    while let Some(((root_level, _), root)) = queue.pop() {
        // Walk the subtree below `root`; an edge to a target at most as deep as
        // `root` is in the frontier.
        let mut worklist = vec![root];
        while let Some(block) = worklist.pop() {
            // The successor in the direction asked: a predecessor for the
            // reverse frontier.
            let around = if POST { graph.predecessors(block) } else { graph.successors(block) };
            for next in around {
                let (next_level, _) = key(next);
                if next_level > root_level || !visited_queue.insert(next) {
                    continue;
                }
                if live_in.is_some_and(|live| !live.contains(&next)) {
                    continue;
                }
                frontier.push(next);
                if !defining.contains(&next) {
                    queue.push((key(next), next));
                }
            }
            for child in tree.children(Some(block)) {
                if visited_worklist.insert(child) {
                    worklist.push(child);
                }
            }
        }
    }
    frontier
}

/// `ForwardIDFCalculator`: where a value defined in `defining` needs phis.
pub fn forward<G: DomGraph>(
    tree: &DomTree,
    graph: &G,
    defining: &IdSet<BlockId>,
    live_in: Option<&IdSet<BlockId>>,
) -> Vec<BlockId> {
    calculate(tree, graph, defining, live_in)
}

/// `ReverseIDFCalculator`: the blocks `defining` is control dependent on.
pub fn reverse<G: DomGraph>(
    tree: &PostDomTree,
    graph: &G,
    defining: &IdSet<BlockId>,
    live_in: Option<&IdSet<BlockId>>,
) -> Vec<BlockId> {
    calculate(tree, graph, defining, live_in)
}

#[cfg(test)]
#[path = "idf_tests.rs"]
mod tests;
