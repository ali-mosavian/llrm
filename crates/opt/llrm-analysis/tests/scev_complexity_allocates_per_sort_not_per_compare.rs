//! `CompareValueComplexity` and `CompareSCEVComplexity` copied each
//! instruction's and each expression's operands into a `Vec` for every
//! comparison (65 K allocations, 60 M of a jpeg_fixed -O2 compile, in the
//! sorts of `GroupByComplexity`). They compare the operands where they lie.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use llrm_analysis::scev::values::print_module_values;

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

/// A sum of 24 call results, each of two operands: sorting them compares the
/// calls' operands pairwise.
#[test]
fn test_sorting_the_terms_of_a_sum_of_calls_allocates_no_operand_copies() {
    let terms = 24;
    let mut text = String::from("declare i32 @g(i32, i32)\n");
    text.push_str("define i32 @f(");
    text.push_str(&(0..terms).map(|at| format!("i32 %p{at}")).collect::<Vec<_>>().join(", "));
    text.push_str(") {\n");
    for at in 0..terms {
        text.push_str(&format!("  %c{at} = call i32 @g(i32 %p{at}, i32 %p{})\n", terms - 1 - at));
    }
    text.push_str("  %s0 = add i32 %c0, %c1\n");
    for at in 2..terms {
        text.push_str(&format!("  %s{} = add i32 %s{}, %c{at}\n", at - 1, at - 2));
    }
    text.push_str(&format!("  ret i32 %s{}\n}}\n", terms - 2));
    let module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    let before = ALLOCATED.with(Cell::get);
    let printed = print_module_values(&module);
    let allocated = ALLOCATED.with(Cell::get) - before;
    assert!(printed.contains("%s22"), "{printed}");
    // 4,322 with a copy per comparison, 2,822 without.
    assert!(allocated < 3500, "{allocated} allocations for the sums of {terms} calls");
}
