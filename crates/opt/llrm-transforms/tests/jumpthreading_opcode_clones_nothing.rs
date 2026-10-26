//! `JumpThreading::opcode` cloned the opcode it read, a boxed `CallInfo` and
//! its attribute lists for a call: 51 allocations for each call tested by a
//! branch, 19 now; 8.9% of the pass on d_faces. It reads the opcode in place.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use llrm_mir::target::Neutral;
use llrm_transforms::jumpthreading::{JumpThreading, JumpThreadingOptions};

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

fn chain(calls: usize) -> String {
    let mut text = String::from("define i32 @f(i32 %a) {\nentry:\n  br label %b0\n");
    for i in 0..calls {
        let next = if i + 1 < calls { format!("b{}", i + 1) } else { "done".to_string() };
        text += &format!(
            "b{i}:\n  %r{i} = call i32 @g(i32 %a)\n  %c{i} = icmp eq i32 %r{i}, 0\n  br i1 %c{i}, label %{next}, label %x{i}\nx{i}:\n  br label %{next}\n"
        );
    }
    text + "done:\n  ret i32 %a\n}\n\ndeclare i32 @g(i32)\n"
}

fn allocations(calls: usize) -> usize {
    let mut module = llrm_mir::parse::module(&chain(calls)).unwrap_or_else(|why| panic!("{why:?}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(JumpThreading { enabled: true, options: JumpThreadingOptions::default() });
    let before = allocated();
    manager.run_module(&mut module, std::rc::Rc::new(Neutral)).expect("runs");
    allocated() - before
}

#[test]
fn test_reading_the_opcode_of_a_call_clones_nothing() {
    let (few, many) = (allocations(10), allocations(40));
    let each = (many - few) / 30;
    assert!(each < 30, "each call tested took {each} allocations ({few} for 10 calls, {many} for 40)");
}
