//! `Function::successors` and `predecessors` made a `Vec` for every ask (42 M
//! of a d_faces -O2 compile): the blocks of a branch, of a switch of up to four
//! arms and of a join of up to four are held in the value returned.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use llrm_mir::parse;

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
fn test_the_successors_and_predecessors_of_a_branch_and_a_join_allocate_nothing() {
    let mut module = parse::module(
        "define i32 @f(i1 %c) {\nentry:\n  br i1 %c, label %yes, label %no\nyes:\n  br label %join\nno:\n  br label %join\njoin:\n  ret i32 0\n}\n",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let f = module.function_mut("f").expect("@f").1;
    let (entry, join) = (f.layout()[0], f.layout()[3]);
    let before = ALLOCATED.with(Cell::get);
    let (mut named, mut joined) = (0, 0);
    for _ in 0..1000 {
        named += f.successors(entry).iter().count();
        joined += f.predecessors(join).iter().count();
    }
    let made = ALLOCATED.with(Cell::get) - before;
    assert_eq!((named, joined), (2000, 2000));
    assert_eq!(made, 0, "{made} allocations for 2,000 asks of a branch's successors and a join's predecessors");
}
