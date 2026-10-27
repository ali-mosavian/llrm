//! A function taking a large struct by value asks whether each of its frame
//! cells meets each store (n^2 asks): each ask built five sets, so 2048 words
//! took 13 s (#830).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use llrm_codegen::backend::overlap::may_overlap;
use llrm_codegen::objectfile::module::{Addr, Space};

struct Counting;

thread_local! {
    /// This thread's allocations: another thread's (the harness's) are no part
    /// of what an ask costs. A `const` cell with no destructor allocates
    /// nothing to be read.
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

#[test]
fn test_asking_whether_two_frame_cells_meet_allocates_nothing() {
    let before = allocated();
    let mut met = 0;
    for a in 0..100_i64 {
        for b in 0..100_i64 {
            let (one, other) = (Some(Addr::new(Space::Frame, a * 4)), Some(Addr::new(Space::Frame, b * 4)));
            met += usize::from(may_overlap(one, 4, other, 4));
        }
    }
    let allocated = allocated() - before;
    assert_eq!(met, 100, "each word meets itself");
    assert_eq!(allocated, 0, "{allocated} allocations to ask 10,000 times whether two frame words meet");
}
