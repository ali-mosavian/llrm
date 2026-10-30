//! The sink's setup made a list of reads for each value and grew its tables a
//! step at a time: 3 allocations a value on d_faces' functions. Where each
//! value is read is one array found by counting.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;

use llrm_codegen::backend::machinesink::sunk;
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

/// Allocations of one `sunk` over a chain of `length` values.
fn allocations(length: u32) -> usize {
    let mut body = chain(length);
    body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
    let before = allocated();
    let moved = sunk(&body, false);
    let count = allocated() - before;
    drop(moved);
    count
}

#[test]
fn test_the_sinks_setup_allocates_no_list_for_each_value() {
    // The first run pays for lazily built tables.
    allocations(4);
    let (few, many) = (allocations(100), allocations(300));
    eprintln!("few {few} many {many}");
    let each = (many as f64 - few as f64) / 200.0;
    assert!(each < 0.5, "{each} allocations a value");
}
