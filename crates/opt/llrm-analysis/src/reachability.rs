//! `isPotentiallyReachable` of `CFG.cpp` at d1106deb71cc: whether a path may
//! lead from one instruction or block to another.
//!
//! # How it works
//! Within one block, `a` reaches `b` if it comes first. Otherwise the walk
//! goes from `a`'s successors (or from `a`'s block) over the graph and answers
//! true at `b`'s block, or at a block that dominates it. The walk visits at
//! most [`MAX_BBS_TO_EXPLORE`] blocks and then answers "maybe", so a caller
//! that asks of every capture pays a bounded price for each.
//!
//! DEVIATE R1: no `LoopInfo`, `CycleInfo` or exclusion set. A block in a cycle
//! reaches an earlier instruction of itself only through the walk, which is
//! the same answer, found by exploring. MIR here has no `CycleInfo`; the
//! dominator tree is the only extra input.
//!
//! # Example
//! In a diamond, neither arm reaches the other; in a loop the body reaches
//! itself:
//!
//! ```mir
//! define void @f(i1 %c) {
//! entry:
//!   br i1 %c, label %left, label %right
//! left:
//!   br label %join
//! right:
//!   br label %join
//! join:
//!   ret void
//! }
//! ```
//!
//! From unit test `a_diamond_arm_does_not_reach_the_other_arm` in
//! `reachability_tests.rs`.
//!
//! # Provides
//! [`is_potentially_reachable`], [`is_potentially_reachable_blocks`],
//! [`is_potentially_reachable_from_many`].
//!
//! # See also
//! LLVM `CFG.cpp`, [`crate::capturetracking`].

use llrm_mir::dense::IdSet;
use llrm_mir::domtree::DomTree;
use llrm_mir::module::{BlockId, Function, InstId};

/// `dom-tree-reachability-max-bbs-to-explore`: blocks explored before the
/// answer is "maybe".
pub const MAX_BBS_TO_EXPLORE: u32 = 32;

/// `isReachableImpl` for one stop block and no exclusion set, from `worklist`.
pub fn is_potentially_reachable_from_many(
    function: &Function,
    mut worklist: Vec<BlockId>,
    stop: BlockId,
    tree: Option<&DomTree>,
) -> bool {
    // An unreachable stop block is dominated from everywhere: the tree says
    // nothing about it.
    let tree = tree.filter(|tree| tree.is_reachable(stop));
    let mut limit = MAX_BBS_TO_EXPLORE;
    let mut visited = IdSet::<BlockId>::new();
    while let Some(block) = worklist.pop() {
        if !visited.insert(block) {
            continue;
        }
        if block == stop || tree.is_some_and(|tree| tree.dominates(block, stop)) {
            return true;
        }
        limit -= 1;
        if limit == 0 {
            // Not settled either way: there is potentially a path.
            return true;
        }
        worklist.extend(function.successors(block).iter().copied());
    }
    false
}

/// `isPotentiallyReachable(const BasicBlock *, const BasicBlock *, ...)`.
pub fn is_potentially_reachable_blocks(
    function: &Function,
    a: BlockId,
    b: BlockId,
    tree: Option<&DomTree>,
) -> bool {
    if let Some(tree) = tree {
        if tree.is_reachable(a) && !tree.is_reachable(b) {
            return false;
        }
        if function.entry() == Some(a) && tree.is_reachable(b) {
            return true;
        }
        if function.entry() == Some(b) && tree.is_reachable(a) {
            return false;
        }
    }
    is_potentially_reachable_from_many(function, vec![a], b, tree)
}

/// `isPotentiallyReachable(const Instruction *, const Instruction *, ...)`.
pub fn is_potentially_reachable(
    function: &Function,
    a: InstId,
    b: InstId,
    tree: Option<&DomTree>,
) -> bool {
    let (block_a, block_b) = (function.parent(a).expect("placed"), function.parent(b).expect("placed"));
    if block_a != block_b {
        return is_potentially_reachable_blocks(function, block_a, block_b, tree);
    }
    // The only case that looks inside a block: once the walk leaves it, the
    // whole block is reachable.
    if a == b || function.comes_before(a, b) {
        return true;
    }
    if function.entry() == Some(block_a) {
        return false;
    }
    let successors: Vec<BlockId> = function.successors(block_a).iter().copied().collect();
    if successors.is_empty() {
        return false;
    }
    is_potentially_reachable_from_many(function, successors, block_b, tree)
}

#[cfg(test)]
#[path = "reachability_tests.rs"]
mod tests;
