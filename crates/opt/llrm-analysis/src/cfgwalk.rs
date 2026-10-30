//! Walks of a function's CFG that LLVM's passes ask for by name.
//!
//! # How it works
//! `find_function_backedges` is `FindFunctionBackedges` (`Analysis/CFG.cpp`): a
//! depth-first search from the entry with three states per block; an edge to a
//! block still on the stack is a back edge. It is cheaper than dominators and
//! loop info, and differs from them on irreducible loops.
//! `inverse_post_order_ext` is `inverse_post_order_ext(BB, Visited)` of
//! `ADT/PostOrderIterator.h`: the post-order of the walk over predecessors from
//! a block, skipping blocks the caller's set has seen, which it fills. Both are
//! linear.
//!
//! # Used by
//! `adce` in llrm-transforms.
//!
//! # Example
//! In a loop the edge to a block still on the stack is the back edge:
//!
//! ```text
//! entry -> head -> body -> head   ; body -> head is the back edge
//! ```

use llrm_mir::dense::IdSet;
use llrm_mir::module::{BlockId, Function};

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/CFG.cpp#L36>
pub fn find_function_backedges(function: &Function) -> Vec<(BlockId, BlockId)> {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Unvisited,
        Visited,
        InStack,
    }
    let Some(entry) = function.entry() else { return Vec::new() };
    let mut state = vec![State::Unvisited; function.block_capacity()];
    let mut result = Vec::new();
    // Each entry: the block and the successors still to look at.
    let mut stack = vec![(entry, function.successors(entry), 0_usize)];
    state[entry.0 as usize] = State::InStack;
    while let Some((block, successors, next)) = stack.last_mut() {
        let block = *block;
        let mut found_new = None;
        while *next < successors.len() {
            let successor = successors[*next];
            *next += 1;
            match state[successor.0 as usize] {
                State::Unvisited => {
                    state[successor.0 as usize] = State::InStack;
                    found_new = Some(successor);
                    break;
                }
                State::InStack => result.push((block, successor)),
                State::Visited => {}
            }
        }
        match found_new {
            Some(successor) => stack.push((successor, function.successors(successor), 0)),
            None => {
                state[block.0 as usize] = State::Visited;
                stack.pop();
            }
        }
    }
    result
}

/// The blocks reached from `start` over predecessors, each after the ones it
/// is reached through, leaving out those in `visited` and adding the rest to
/// it: `inverse_post_order_ext(&BB, Visited)`.
pub fn inverse_post_order_ext(
    function: &Function,
    start: BlockId,
    visited: &mut IdSet<BlockId>,
) -> Vec<BlockId> {
    let mut order = Vec::new();
    if !visited.insert(start) {
        return order;
    }
    let walk = |block: BlockId| function.predecessors(block).iter().copied().collect::<Vec<_>>();
    let mut stack = vec![(start, walk(start), 0_usize)];
    while let Some((block, predecessors, next)) = stack.last_mut() {
        if let Some(&predecessor) = predecessors.get(*next) {
            *next += 1;
            if visited.insert(predecessor) {
                stack.push((predecessor, walk(predecessor), 0));
            }
        } else {
            order.push(*block);
            stack.pop();
        }
    }
    order
}

#[cfg(test)]
#[path = "cfgwalk_tests.rs"]
mod tests;
