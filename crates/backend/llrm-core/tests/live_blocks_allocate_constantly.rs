//! The blocks a value is seen in and the blocks it only passes through were
//! B-trees of block positions, a node for every eleven blocks: `analysed_in`
//! was 17 M of a d_faces -O2 compile, 16 M of it in the trees. They are bit
//! sets: an allocation each, whatever the blocks.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;

use llrm_core::backend::allocate::LiveAt;
use llrm_core::backend::splitkit::live_blocks;
use llrm_core::model::ir::{Held, Imm, Loc, Operation, Semantics};
use llrm_core::model::lir::{Block, Insn, LirBlock, LirBody};
use llrm_core::support::hash::IndexMap;

struct Counting;

thread_local! {
    /// This thread's allocations: another test's are no part of what a call
    /// costs.
    static ALLOCATED: Cell<usize> = const { Cell::new(0) };
}

fn allocated() -> usize {
    ALLOCATED.with(Cell::get)
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        ALLOCATED.with(|count| count.set(count.get() + 1));
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

/// Live into every block but the first and out of every block but the last.
struct Across {
    first: i64,
    last: i64,
}

impl LiveAt for Across {
    fn live_in(
        &self,
        block: i64,
        _: u32,
    ) -> bool {
        block != self.first
    }

    fn live_out(
        &self,
        block: i64,
        _: u32,
    ) -> bool {
        block != self.last
    }
}

fn chain(blocks: i64) -> LirBody {
    let held = Loc::Held(Held { value: 3, width: 2 });
    let made = Semantics {
        name: Some("mov".to_owned()),
        dests: vec![held.clone()],
        sources: vec![Loc::Imm(Imm { value: 1, width: 2, address: None })],
        ..Semantics::new(Operation::Move)
    };
    let read = Semantics { name: Some("push".to_owned()), sources: vec![held], ..Semantics::new(Operation::Push) };
    let all: Vec<Block> = (0..blocks)
        .map(|k| {
            let at = k * 0x10;
            let mut insns = Vec::new();
            if k == 0 {
                insns.push(Arc::new(Insn::new(at, Some((at, at + 2)), Some(made.clone()), vec![3], vec![])));
            }
            if k == blocks - 1 {
                insns.push(Arc::new(Insn::new(at + 1, Some((at + 1, at + 2)), Some(read.clone()), vec![], vec![3])));
            }
            let succ = if k == blocks - 1 { vec![] } else { vec![(k + 1) * 0x10] };
            Block::from(LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) })
        })
        .collect();
    LirBody::new("chain", 0, all, IndexMap::default(), IndexMap::default())
}

#[test]
fn test_the_blocks_a_value_passes_through_are_bit_sets_not_a_node_for_eleven_blocks() {
    let blocks = 400;
    let body = chain(blocks);
    let live = Across { first: 0, last: (blocks - 1) * 0x10 };
    // The body's edges are a kept fact, made by the first ask.
    live_blocks(&body, 3, &live);
    let before = allocated();
    let counted = live_blocks(&body, 3, &live);
    let made = allocated() - before;
    assert_eq!(counted as i64, blocks, "premise: the two blocks that name it and every block between");
    assert!(made < 40, "{made} allocations for a value live across {blocks} blocks");
}
