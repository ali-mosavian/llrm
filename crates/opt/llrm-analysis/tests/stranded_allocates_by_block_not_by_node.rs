//! The blocks that cannot return were a B-tree of block addresses, a node
//! for every handful of blocks: `stranded` was 10 M of a d_faces -O2 compile,
//! 8 M of it in the tree. They are a bit set, an allocation for the whole.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use llrm_analysis::cfg;
use llrm_analysis::noreturn::stranded;
use llrm_analysis::testing::DOS;

struct Counting;

thread_local! {
    /// This thread's allocations: another test's are no part of what a call
    /// costs.
    static ALLOCATED: Cell<usize> = const { Cell::new(0) };
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

/// 1,000 blocks in a chain that ends in `unreachable`: all but the header are
/// stranded.
#[test]
fn test_stranded_costs_an_allocation_for_the_graph_and_none_for_each_stranded_block() {
    let blocks = 1000;
    let mut text = format!("{DOS}define void @f() {{\nb0:\n  br label %b1\n");
    for at in 1..blocks - 1 {
        text.push_str(&format!("b{at}:\n  br label %b{}\n", at + 1));
    }
    text.push_str(&format!("b{}:\n  unreachable\n}}\n", blocks - 1));
    let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    let function = module.function_mut("f").unwrap().1;
    let before = ALLOCATED.with(Cell::get);
    let found = stranded(function, cfg::id(function.entry().unwrap()));
    let allocated = ALLOCATED.with(Cell::get) - before;
    assert_eq!(found.len(), blocks - 1);
    // `cfg::graph` allocates a successor list for each block: the floor (1,008;
    // the B-tree took 1,107).
    assert!(allocated < blocks + 50, "{allocated} allocations for {blocks} blocks");
}
