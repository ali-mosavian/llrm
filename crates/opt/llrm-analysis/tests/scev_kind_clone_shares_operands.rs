//! `Kind::clone` copied an `Add`/`Mul`/`AddRec` expression's operand `Vec`
//! (49 M of a memory_expansion -O2 compile in `Kind::clone` alone). Operands
//! are shared, so a clone allocates nothing.

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

/// A loop whose index feeds 96 address sums: each sum is rebuilt, folded and
/// matched, cloning its operands every time.
#[test]
fn test_wide_sums_of_a_loop_index_are_not_copied_per_visit() {
    let terms = 96;
    let mut text = String::from("define i64 @f(i64 %n");
    for at in 0..terms {
        text.push_str(&format!(", i64 %p{at}"));
    }
    text.push_str(") {\nentry:\n  br label %loop\nloop:\n");
    text.push_str("  %i = phi i64 [ 0, %entry ], [ %next, %loop ]\n");
    text.push_str("  %s0 = add i64 %i, %p0\n");
    for at in 1..terms {
        text.push_str(&format!("  %s{at} = add i64 %s{}, %p{at}\n", at - 1));
    }
    text.push_str("  %next = add i64 %i, 1\n  %done = icmp eq i64 %next, %n\n");
    text.push_str("  br i1 %done, label %exit, label %loop\nexit:\n");
    text.push_str(&format!("  ret i64 %s{}\n}}\n", terms - 1));
    let module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    let before = ALLOCATED.with(Cell::get);
    let printed = print_module_values(&module);
    let allocated = ALLOCATED.with(Cell::get) - before;
    assert!(printed.contains(&format!("%s{}", terms - 1)), "{printed}");
    // 48,435 with a copy per clone, 46,678 without.
    assert!(allocated < 47_500, "{allocated} allocations for {terms} sums");
}
