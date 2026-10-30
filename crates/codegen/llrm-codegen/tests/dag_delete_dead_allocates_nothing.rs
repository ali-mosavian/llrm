//! `delete_dead` started a worklist with `vec![node]`: one allocation per call,
//! 51K on jpeg_fixed -O2 (3% of the backend's allocations). Asking to delete a
//! node that is already gone, or one that has users, allocates nothing.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use llrm_codegen::backend::selectiondag::{DagConfig, EVT, ISD, SelectionDAG};

struct Counting;

thread_local! {
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

#[test]
fn test_deleting_a_node_that_stays_allocates_nothing() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let (a, b) = (dag.get_register(EVT::I32, 1), dag.get_register(EVT::I32, 2));
    let sum = dag.get_node(ISD::Add, EVT::I32, &[a, b]);
    dag.set_root(sum);
    dag.delete_dead(sum.node);
    let before = ALLOCATED.with(Cell::get);
    for _ in 0..1000 {
        dag.delete_dead(sum.node);
    }
    let allocated = ALLOCATED.with(Cell::get) - before;
    assert_eq!(allocated, 0, "{allocated} allocations for 1,000 deletions that did nothing");
}
