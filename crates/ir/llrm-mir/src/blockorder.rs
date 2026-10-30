//! `ReversePostOrderTraversal<Function *>`: the blocks a function's entry
//! reaches, each before its successors but for back edges.
//!
//! Translation of LLVM's
//! [`ReversePostOrderTraversal`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/PostOrderIterator.h#L292)
//! over `po_iterator`
//! ([:84](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/PostOrderIterator.h#L84))
//! at d1106deb71cc.
//!
//! MIRROR: a depth-first walk from the entry, the first successor first, with
//! the blocks listed as they are left and the list reversed. A block no path
//! from the entry reaches is not in it.
//!
//! # How it works
//! One walk with an explicit stack and a set of the blocks seen: O(blocks +
//! edges).
//!
//! Not this: `llrm_support::graph::reverse_postorder` lists the blocks the
//! entry cannot reach after the reachable ones (a dataflow worklist wants every
//! block), and the older passes that read that order keep it. A pass that
//! mirrors an LLVM loop over a `ReversePostOrderTraversal` uses this one: a
//! block no path reaches may hold what LLVM never lets a pass see (an
//! instruction that reads itself, outside a phi).
//!
//! # Used by
//! `reassociate`, and the translated passes that walk the blocks in this order.
//!
//! # Example
//! `a` branches to `b` and `c`, which both go to `d`: `a c b d`.

use crate::dense::IdSet;
use crate::module::{BlockId, Function};

/// The blocks `function`'s entry reaches, in reverse post-order; none for a
/// declaration.
pub fn reverse_post_order_traversal(function: &Function) -> Vec<BlockId> {
    let Some(entry) = function.entry() else { return Vec::new() };
    // The successors still to walk, the first on top.
    let rest = |block: BlockId| {
        let mut successors: Vec<BlockId> = function.successors(block).into_iter().collect();
        successors.reverse();
        successors
    };
    let mut seen = IdSet::<BlockId>::new();
    seen.insert(entry);
    let mut post: Vec<BlockId> = Vec::new();
    let mut stack = vec![(entry, rest(entry))];
    while let Some((block, successors)) = stack.last_mut() {
        match successors.pop() {
            Some(next) if seen.insert(next) => stack.push((next, rest(next))),
            Some(_) => {}
            None => {
                post.push(*block);
                stack.pop();
            }
        }
    }
    post.reverse();
    post
}

#[cfg(test)]
#[path = "blockorder_tests.rs"]
mod tests;
