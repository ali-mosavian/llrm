//! The first `intrinsic_cost` call parsed the 1057-line price text into a
//! hash map: 1.06 M Ir on d_faces -O2, once per process. A lookup allocates
//! nothing. Alone in its binary: the old table was built once per process, by
//! whichever test came first.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use llrm_mir::tti::{Cost, CostKind, Tti};
use llrm_x86::tti::X86;

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
fn test_the_first_intrinsic_price_builds_no_table() {
    let x86 = X86::of_cpu("i486").unwrap();
    let before = ALLOCATED.with(Cell::get);
    let cost = x86.intrinsic_cost("llvm.abs.i32", None, &[], CostKind::CodeSize);
    let after = ALLOCATED.with(Cell::get);
    assert_eq!(cost, Cost::of(3));
    assert_eq!(after - before, 0, "a lookup allocated");
}
