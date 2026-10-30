//! `combine`'s visit cloned the instruction's `Opcode` to look up its visitors,
//! a `Box<CallInfo>` allocation for each call it visited: 0.9 M Ir on d_faces.
//! The table is `'static`; a call costs the combine no allocation of its own.

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

/// Allocations of one `combine` over `@f` holding `calls` calls of `@g`.
fn allocations(calls: usize) -> usize {
    let mut text = String::from("declare void @g(i32)\ndefine void @f(i32 %a) {\nentry:\n");
    for _ in 0..calls {
        text += "  call void @g(i32 %a)\n";
    }
    text += "  ret void\n}\n";
    let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|why| panic!("{why:?}"));
    let mut manager = PassManager::default();
    manager.add(Combine { enabled: true });
    let before = ALLOCATED.with(Cell::get);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
    ALLOCATED.with(Cell::get) - before
}

#[test]
fn test_a_call_costs_the_combine_no_allocation_of_its_own() {
    // The first run pays for lazily built tables.
    allocations(1);
    let (few, many) = (allocations(100), allocations(300));
    eprintln!("few {few} many {many}");
    let each = (many as f64 - few as f64) / 200.0;
    assert!(each < 0.5, "{each} allocations per call");
}
