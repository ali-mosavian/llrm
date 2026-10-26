//! Old: `reverse_postorder` kept its blocks in a `BTreeMap` and its seen set in
//! a `BTreeSet`, a node allocation for about every five blocks and a descent
//! for every edge (1.2 M Ir in the 50 InstCombine runs of d_faces -O2). Its
//! allocations are now a few, growing with the log of the depth,; its order is
//! the same.

use llrm_support::alloc_count::{Counting, counted};
use llrm_support::graph::{Node, reverse_postorder};

#[global_allocator]
static COUNTING: Counting = Counting;

struct Block {
    at: i64,
    succ: Vec<i64>,
}

impl Node for Block {
    fn at(&self) -> i64 {
        self.at
    }

    fn succ(&self) -> &[i64] {
        &self.succ
    }
}

fn block(
    at: i64,
    succ: &[i64],
) -> Block {
    Block { at, succ: succ.to_vec() }
}

#[test]
fn test_the_order_is_reverse_postorder_then_the_unreachable_in_body_order() {
    // 1 -> 2 -> 4, 1 -> 3 -> 4, 4 -> 2 (back edge); 9 -> 1 and 7 are
    // unreachable; 5 is named only.
    let blocks = [block(1, &[2, 3]), block(2, &[4]), block(3, &[4, 5]), block(4, &[2]), block(9, &[1]), block(7, &[])];
    assert_eq!(reverse_postorder(&blocks, 1), [1, 3, 2, 4, 9, 7]);
    // An entry that is no block reaches nothing: every block, in body order.
    assert_eq!(reverse_postorder(&blocks, 100), [1, 2, 3, 4, 9, 7]);
}

#[test]
fn test_allocations_do_not_grow_with_the_blocks() {
    let blocks: Vec<Block> = (0..4000).map(|at| block(at, &[at + 1, at + 2])).collect();
    let (order, made) = counted(|| reverse_postorder(&blocks, 0));
    assert_eq!(order.len(), 4000);
    assert!(made <= 40, "{made} allocations for 4000 blocks");
}

/// The first version, to compare orders with.
fn reference(
    blocks: &[Block],
    entry: i64,
) -> Vec<i64> {
    use std::collections::{BTreeMap, BTreeSet};
    let known = blocks.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::from([entry]);
    let mut post = Vec::new();
    let mut stack = known.get(&entry).map(|block| vec![(*block, 0usize)]).unwrap_or_default();
    while let Some((block, next)) = stack.last_mut() {
        let block = *block;
        if let Some(successor) = block.succ.get(*next) {
            *next += 1;
            if let Some(child) = known.get(successor).filter(|_| seen.insert(*successor)) {
                stack.push((*child, 0));
            }
        } else {
            post.push(block.at);
            stack.pop();
        }
    }
    post.reverse();
    post.extend(blocks.iter().map(|block| block.at).filter(|at| !seen.contains(at)));
    post
}

#[test]
fn test_the_order_is_the_first_versions_on_many_graphs() {
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = move |bound: u64| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state % bound
    };
    for _ in 0..300 {
        let count = 1 + next(12) as i64;
        let blocks: Vec<Block> = (0..count)
            .map(|at| {
                let succ: Vec<i64> = (0..next(4)).map(|_| next(count as u64 + 2) as i64).collect();
                Block { at: at * 3, succ: succ.into_iter().map(|to| to * 3).collect() }
            })
            .collect();
        let entry = next(count as u64 + 1) as i64 * 3;
        assert_eq!(reverse_postorder(&blocks, entry), reference(&blocks, entry), "entry {entry}");
    }
}
