//! `register_extra_bytes` named the register by formatting its `Debug` into a
//! `String` on every call: 42,678 calls and 800 instructions each (1.1% of
//! the compile of memory_expansion at -O2).

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use llrm_lir::registers::RegId;
use llrm_lir::{Held, Loc, Operation, Semantics};
use llrm_x86::register_cost::register_extra_bytes;

struct Counting;
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
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
fn pricing_a_register_allocates_nothing() {
    let held = Loc::Held(Held { value: 7, width: 1, offset: 0 });
    let what = Semantics {
        op: Operation::Move,
        name: None,
        dests: vec![held],
        sources: vec![],
        target: None,
        indirect: false,
        table: vec![],
        table_id: None,
        repeat: false,
        pops: None,
    };
    let registers: Vec<RegId> = RegId::values().collect();
    let sum = |registers: &[RegId]| registers.iter().map(|&reg| register_extra_bytes(&what, 7, reg)).sum::<i64>();
    let first = sum(&registers);
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    let again = sum(&registers);
    let made = ALLOCATIONS.load(Ordering::Relaxed) - before;
    assert_eq!(first, again);
    assert_eq!(made, 0, "{made} allocations pricing {} registers", registers.len());
}
