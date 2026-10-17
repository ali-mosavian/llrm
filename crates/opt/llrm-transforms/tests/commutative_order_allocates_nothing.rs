//! `transform::_computation` ordered the two operands of a commutative
//! operation by their `Debug` text, two strings for each: 12.6k allocations on
//! d_faces -Os. The order is the operands' own.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::BTreeMap;

use llrm_mir::module::InstId;

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
fn test_ordering_the_operands_of_a_commutative_operation_allocates_nothing() {
    let parsed = llrm_mir::parse::module(
        "define i32 @f(i32 %a, i32 %b) {\nentry:\n  %x = add i32 %b, %a\n  %y = sub i32 %b, %a\n  %z = add i32 %a, %b\n  ret i32 %z\n}\n",
    );
    let module = parsed.unwrap_or_else(|why| panic!("{why:?}"));
    let function = module.named("f").and_then(|id| module.global(id).function()).expect("@f");
    let (swapped, plain, sub) = {
        let find = |name: &str| {
            function
                .walk()
                .map(|(_, inst)| inst)
                .find(|&inst| {
                    function.instruction(inst).result.and_then(|r| function.value(r).name.clone()).as_deref()
                        == Some(name)
                })
                .expect("a named instruction")
        };
        (find("x"), find("z"), find("y"))
    };
    let stands = BTreeMap::new();
    let measure = |inst: InstId| {
        let before = allocated();
        let found = llrm_transforms::transform::_computation(function.instruction(inst), &stands);
        (allocated() - before, found)
    };
    let (add_allocations, add) = measure(swapped);
    let (_, again) = measure(plain);
    let (sub_allocations, _) = measure(sub);
    assert!(add.is_some() && add == again, "a + b and b + a are one computation");
    assert_eq!(
        add_allocations,
        sub_allocations,
        "ordering the operands of an add took {} allocations more than not ordering a sub's",
        add_allocations as i64 - sub_allocations as i64
    );
}
