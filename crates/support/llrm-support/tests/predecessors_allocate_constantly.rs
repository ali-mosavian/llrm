//! `predecessors` made a set for every block and a node for every eleven
//! predecessors: 20 M of a d_faces -O2 compile, 16 M of it in the trees. Its
//! lists are one array, so the allocations do not grow with the blocks.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use llrm_support::graph::{Node, predecessors};

struct Counting;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        ALLOCATED.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(
        &self,
        pointer: *mut u8,
        layout: Layout,
    ) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

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

/// Block k goes to k + 1 and to the join at `blocks`; a branch to a block that
/// is not there is dropped, a repeated edge counts once.
fn diamond(blocks: i64) -> Vec<Block> {
    let mut out: Vec<Block> = (0..blocks)
        .map(|at| Block { at: at * 2, succ: vec![(at + 1) * 2, blocks * 2, blocks * 2, 9_999_999] })
        .collect();
    out.push(Block { at: blocks * 2, succ: vec![] });
    out
}

#[test]
fn test_the_predecessors_of_a_thousand_blocks_are_a_few_allocations() {
    let blocks = diamond(1000);
    let before = ALLOCATED.load(Ordering::Relaxed);
    let found = predecessors(&blocks);
    let made = ALLOCATED.load(Ordering::Relaxed) - before;
    assert_eq!(found.len(), 1001);
    assert_eq!(found[&2000].len(), 1000, "every block but the join goes to the join, once");
    assert_eq!(found[&2].iter().copied().collect::<Vec<_>>(), vec![0], "block 0 goes to block 2");
    assert!(found.get(&9_999_999).is_none(), "a block that is not there has no predecessors");
    assert!(made <= 8, "{made} allocations for the predecessors of 1,001 blocks");
}
