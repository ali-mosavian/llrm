//! The 68020 tables parse once, with one allocation per cell they keep: a
//! parse that cloned every field and rebuilt every key cost ~0.4M instructions
//! (6115 allocations, now 4534) of a 123M compile (grep, -O2) before the first
//! instruction was priced.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
    ) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static A: Counting = Counting;

#[test]
fn the_first_price_on_a_68020_allocates_no_more_than_the_tables_it_keeps() {
    let before = ALLOCS.load(Ordering::Relaxed);
    llrm_m68k::timing::cost_on(llrm_m68k::isa::Cpu::M68020, "move.l %d1, %d0", 2, false).unwrap();
    llrm_m68k::timing::cost_on(llrm_m68k::isa::Cpu::M68020, "add.l (%a0), %d0", 2, false).unwrap();
    let made = ALLOCS.load(Ordering::Relaxed) - before;
    assert!(made <= LIMIT, "{made} allocations to build the 68020 tables, limit {LIMIT}");
}

/// 6115 before the fix, 4534 after.
const LIMIT: usize = 5000;
