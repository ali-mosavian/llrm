//! The values an operand names are asked of for every operand of every
//! instruction, at every rebuild of the allocator's facts; each ask made a
//! vector.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use llrm_lir::{Held, Loc, Mem, values};

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
fn test_asking_what_an_operand_names_allocates_nothing() {
    let held = Held { value: 7, width: 2 };
    let cell = Mem {
        base: Some(Held { value: 1, width: 2 }),
        index: Some(Held { value: 2, width: 2 }),
        selector: Some(Held { value: 3, width: 2 }),
        ..Mem::new(None, 2)
    };
    let operands = [Loc::Held(held), Loc::Mem(cell)];
    let before = ALLOCATED.load(Ordering::Relaxed);
    let mut named = 0;
    for _ in 0..1000 {
        for operand in &operands {
            named += values(operand).into_iter().count();
            named += values(operand).iter().filter(|one| one.value > 1).count();
        }
    }
    let allocated = ALLOCATED.load(Ordering::Relaxed) - before;
    assert_eq!(named, 1000 * (1 + 1 + 3 + 2));
    assert_eq!(allocated, 0, "{allocated} allocations to ask 4,000 times what an operand names");
}
