//! `Profile::cost` built a map of the whole table for every instruction form it
//! was asked: fpbench at -O0 spent 8% of its instructions in that map's
//! inserts.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use llrm_codegen::backend::cpu::tuned_for;

struct Counting;

thread_local! {
    /// This thread's allocations: another thread's (the harness's, a test's own
    /// helper) are no part of what a lookup costs. A `const` cell with no
    /// destructor allocates nothing to be read.
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
fn test_asking_a_profile_for_a_form_allocates_nothing() {
    let profile = tuned_for(&llrm_x86_m16::M16, "486", false).expect("a profile");
    let before = allocated();
    let mut priced = 0;
    for _ in 0..1000 {
        priced += usize::from(profile.cost("alu_rr").is_ok())
            + usize::from(profile.prices("lea"))
            + usize::from(profile.latency("alu_rr").is_ok());
    }
    let allocated = allocated() - before;
    assert_eq!(priced, 3000, "the forms are listed");
    assert_eq!(allocated, 0, "{allocated} allocations to ask 3,000 times for a form");
}

/// The counter was one for the process: a thread allocating beside the lookups
/// (CI's harness thread, "4 allocations to ask 3,000 times for a form") failed
/// a test whose lookups allocate nothing.
#[test]
fn test_another_threads_allocations_are_not_those_of_the_lookups() {
    let profile = tuned_for(&llrm_x86_m16::M16, "486", false).expect("a profile");
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let beside = {
        let stop = std::sync::Arc::clone(&stop);
        std::thread::spawn(move || {
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                std::hint::black_box(vec![0u8; 64]);
            }
        })
    };
    let before = allocated();
    for _ in 0..200_000 {
        std::hint::black_box(profile.cost("alu_rr").is_ok());
    }
    let counted = allocated() - before;
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    beside.join().unwrap();
    assert_eq!(counted, 0, "{counted} allocations of another thread counted");
}
