//! `sink_region` collected the users of every instruction of a loop into a
//! `Vec` to ask whether it had any (1.0 M Ir of a jpeg_fixed -O2 compile).
//! Two allocations fewer per instruction (664 -> 538 over sixty adds).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use llrm_transforms::licm::Licm;

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

/// Allocations of `Licm` over a loop whose body holds `n` adds, each used by
/// the store after it, so each is asked for its users and none is hoisted.
fn allocations(n: usize) -> usize {
    let mut body = String::new();
    for k in 0..n {
        body += &format!("  %x{k} = add i16 %i, {k}\n  store i16 %x{k}, ptr @g\n");
    }
    let text = format!(
        "@g = global i16 0\ndefine void @f(i16 %n) {{\nb0:\n  br label %head\nhead:\n  %i = phi i16 [ 0, %b0 ], [ %j, %head ]\n{body}  %j = add i16 %i, 1\n  %t = icmp slt i16 %j, %n\n  br i1 %t, label %head, label %exit\nexit:\n  ret void\n}}\n"
    );
    let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|why| panic!("{why:?}"));
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.add(Licm { apply: true, speculate: true });
    let before = ALLOCATED.with(Cell::get);
    passes.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("licm");
    ALLOCATED.with(Cell::get) - before
}

#[test]
fn test_sixty_more_instructions_in_a_loop_cost_two_fewer_allocations_each() {
    allocations(2);
    let (small, large) = (allocations(10), allocations(70));
    assert!(large - small < 600, "{small} allocations for 10 adds, {large} for 70");
}
