//! A function taking a large struct by value asks whether each of its frame cells meets each store (n^2 asks): each
//! ask built five sets, so 2048 words took 13 s (#830).

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use llrm_core::backend::overlap::may_overlap;
use llrm_core::objectfile::module::{Addr, Space};

struct Counting;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

#[test]
fn test_asking_whether_two_frame_cells_meet_allocates_nothing() {
    let before = ALLOCATED.load(Ordering::Relaxed);
    let mut met = 0;
    for a in 0..100_i64 {
        for b in 0..100_i64 {
            let (one, other) = (Some(Addr::new(Space::Frame, a * 4)), Some(Addr::new(Space::Frame, b * 4)));
            met += usize::from(may_overlap(one, 4, other, 4));
        }
    }
    let allocated = ALLOCATED.load(Ordering::Relaxed) - before;
    assert_eq!(met, 100, "each word meets itself");
    assert_eq!(allocated, 0, "{allocated} allocations to ask 10,000 times whether two frame words meet");
}
