//! Building the intervals of a body made a list for each value, another for
//! each instruction's slots in it, and grew every table a step at a time: 1289
//! allocations for 400 values. A value costs the one list that holds its
//! operands.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;

use llrm_codegen::liveness::liveintervals::LiveIntervals;
use llrm_codegen::model::ir::{Held, Loc, Operation, Semantics};
use llrm_codegen::model::lir::{Insn, LirBlock, LirBody};

struct Counting;

thread_local! {
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

/// `v1 := mov 1; v2 := add v1, v1; v3 := add v2, v2; ...`
fn chain(length: u32) -> LirBody {
    let held = |value| Loc::Held(Held { value, width: 4 });
    let insns: Vec<Arc<Insn>> = (1..=length)
        .map(|value| {
            let what = Semantics {
                name: Some("add".into()),
                dests: vec![held(value)],
                sources: vec![held(value.max(2) - 1), held(value.max(2) - 1)],
                ..Semantics::new(Operation::Binary)
            };
            let uses = if value == 1 { vec![] } else { vec![value - 1] };
            Arc::new(Insn::new(i64::from(value), None, Some(what), vec![value], uses))
        })
        .collect();
    LirBody::new("chain", 0, vec![LirBlock::new(0, insns).into()], Default::default(), Default::default())
}

#[test]
fn test_building_the_intervals_costs_an_allocation_a_value_not_three_an_operand() {
    let body = chain(400);
    let before = allocated();
    let built = LiveIntervals::new(&body);
    let allocated = allocated() - before;
    drop(built);
    assert!(allocated < 600, "{allocated} allocations to build the intervals of 400 values");
}
