//! `canEvaluateTruncated` kept its visited values in a paged `IdMap`: each
//! trunc allocated a page and a page table the size of the function, 1.8 k Ir
//! self per call against the pin's 0.9 k in all. It keeps them in a small set.
//! A trunc of a load cost 6 allocations beyond a plain store; now 4.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use llrm_mir::passes::PassManager;
use llrm_transforms::combine_pass::Combine;

struct Counting;

thread_local! {
    /// This thread's allocations alone; a `const` cell with no destructor
    /// allocates nothing to be read.
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

/// Allocations of one `combine` over `@f` holding `n` truncs of a load.
fn allocations(n: usize) -> usize {
    allocations_of(n, "  %t{k} = trunc i32 %l{k} to i16\n  store i16 %t{k}, ptr %q\n")
}

fn allocations_of(
    n: usize,
    tail: &str,
) -> usize {
    let mut text = String::from("define void @f(ptr %p, ptr %q) {\nentry:\n");
    for k in 0..n {
        text += &format!("  %l{k} = load i32, ptr %p\n{}", tail.replace("{k}", &k.to_string()));
    }
    text += "  ret void\n}\n";
    let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|why| panic!("{why:?}"));
    let mut manager = PassManager::default();
    manager.add(Combine { enabled: true, phi_constant_users: false });
    let before = ALLOCATED.with(Cell::get);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
    ALLOCATED.with(Cell::get) - before
}

#[test]
fn test_a_trunc_costs_the_evaluator_no_allocation() {
    allocations(1);
    let (few, many) = (allocations(100), allocations(300));
    let each = (many as f64 - few as f64) / 200.0;
    let (few, many) =
        (allocations_of(100, "  store i32 %l{k}, ptr %q\n"), allocations_of(300, "  store i32 %l{k}, ptr %q\n"));
    let plain = (many as f64 - few as f64) / 200.0;
    eprintln!("each {each} plain {plain}");
    assert!(each - plain < 5.0, "{} allocations per trunc beyond a plain store", each - plain);
}
