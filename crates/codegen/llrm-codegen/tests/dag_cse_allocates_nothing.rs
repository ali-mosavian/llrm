//! Asking for a node that exists built an owned key of two vectors to look it
//! up: an allocation pair per ask, 2.9 M Ir of hashing and 2.0 M in `intern`
//! on d_faces -O2.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use llrm_codegen::backend::selectiondag::{DagConfig, EVT, SelectionDAG};

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
fn test_asking_for_an_existing_node_allocates_nothing() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let (undef, five) = (dag.get_undef(EVT::I32), dag.get_constant(5, EVT::I32));
    let before = ALLOCATED.with(Cell::get);
    for _ in 0..1000 {
        assert_eq!(dag.get_undef(EVT::I32), undef);
        assert_eq!(dag.get_constant(5, EVT::I32), five);
    }
    let allocated = ALLOCATED.with(Cell::get) - before;
    assert_eq!(allocated, 0, "{allocated} allocations to find 2,000 nodes that exist");
}
