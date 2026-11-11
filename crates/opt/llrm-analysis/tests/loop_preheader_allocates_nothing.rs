//! `Loop::preheader` collected the header's entering blocks into a `BTreeSet`
//! and a `Vec`, then its successors into another (1.4 K Ir a call, 4297 calls
//! in a memory_expansion -O2 compile). It reads the header's predecessors and
//! the one block's successors where they are.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use llrm_analysis::cfg::{self, Around};
use llrm_analysis::graph::loops;
use llrm_analysis::testing::DOS;

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

const LOOP: &str = "define i32 @f(i32 %n) {
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %j, %b1 ]
  %j = add i32 %i, 1
  %t = icmp slt i32 %j, %n
  br i1 %t, label %b1, label %b2

b2:
  ret i32 %j
}
";

#[test]
fn test_the_preheader_of_a_loop_is_found_without_allocating() {
    let module = llrm_mir::parse::module(&format!("{DOS}{LOOP}")).unwrap_or_else(|error| panic!("{error}"));
    let function = llrm_analysis::testing::function(&module, "f");
    let found = loops::loops(&cfg::graph(function), None);
    let [the_loop] = &found[..] else { panic!("one loop") };
    let before = ALLOCATED.with(Cell::get);
    let preheader = the_loop.preheader(function);
    let allocated = ALLOCATED.with(Cell::get) - before;
    assert_eq!(preheader, Some(cfg::id(function.layout()[0])));
    assert_eq!(allocated, 0, "{allocated} allocations to find the one block that enters a loop");
}
