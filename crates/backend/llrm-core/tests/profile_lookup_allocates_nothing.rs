//! `Profile::cost` built a map of the whole table for every instruction form it was asked: fpbench at -O0 spent 8% of
//! its instructions in that map's inserts.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use llrm_core::backend::cpu::tuned_for;

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

#[test]
fn test_asking_a_profile_for_a_form_allocates_nothing() {
    let profile = tuned_for(&llrm_x86_m16::M16, "486", false).expect("a profile");
    let before = ALLOCATED.load(Ordering::Relaxed);
    let mut priced = 0;
    for _ in 0..1000 {
        priced += usize::from(profile.cost("alu_rr").is_ok())
            + usize::from(profile.prices("lea"))
            + usize::from(profile.latency("alu_rr").is_ok());
    }
    let allocated = ALLOCATED.load(Ordering::Relaxed) - before;
    assert_eq!(priced, 3000, "the forms are listed");
    assert_eq!(allocated, 0, "{allocated} allocations to ask 3,000 times for a form");
}
