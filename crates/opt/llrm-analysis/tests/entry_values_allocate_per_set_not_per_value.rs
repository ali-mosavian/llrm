//! `liveness::entry_values` built two `BTreeSet<ValueId>`s of every value the
//! body defines and reads, a tree node for every few values: 579 allocations
//! for a body of 2,000 values. The sets are `IdSet`s now, whose allocations
//! grow with the log of the largest id, not with the number of values.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    /// This thread's allocations alone; a `const` cell with no destructor
    /// allocates nothing to be read.
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
fn test_the_values_a_body_is_given_are_found_with_a_few_allocations_however_many_it_defines() {
    let mut text = String::from("define i32 @f(i32 %a, i32 %b) {\nentry:\n  %v0 = add i32 %a, %b\n");
    for at in 1..2000 {
        text += &format!("  %v{at} = add i32 %v{}, %a\n", at - 1);
    }
    text += "  ret i32 %v1999\n}\n";
    let module = llrm_mir::parse::module(&text).unwrap_or_else(|why| panic!("{why:?}"));
    let function = module.named("f").and_then(|id| module.global(id).function()).expect("@f");
    let before = allocated();
    let arriving = llrm_analysis::liveness::entry_values(function);
    let spent = allocated() - before;
    assert_eq!(arriving.iter().count(), 2, "the two parameters");
    assert!(spent <= 16, "{spent} allocations for 2,000 values");
}
