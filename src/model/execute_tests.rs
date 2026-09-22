//! Ports of `tests/test_mir_execute.py`.
//!
//! Skipped, needing the modern frontend:
//! `test_frontend_mir_and_its_optimized_form_compute_the_same_sum`,
//! `test_an_unmodelled_operation_raises_rather_than_guessing`.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use num_bigint::BigInt;

use super::{Memory, run};
use crate::model::ir::Operation;
use crate::model::mir::{self, Arg, Cell, Const, FrameAddress, Kind, MemRef, MirBlock, MirBody, Op, OpCode, Value};
use crate::objectfile::module::{Addr, Space};

const POINTER: Value = Value { id: 1, at: 0, flags: false, variable: 0, version: 0 };

/// `POINTER = pointer`, store 0x1234 at `stored`, return what `read` holds.
fn _through_pointer(pointer: Arg, stored: MemRef, read: MemRef, stack_in_data: bool) -> MirBody {
    let take = mir::computed(0, Kind::Copy, POINTER, vec![pointer], 2);
    let mut store = Op::new(0, OpCode::Operation(Operation::Move), "", vec![], vec![POINTER]);
    store.stores = vec![stored.clone()];
    store.kind = Kind::Store;
    store.args = vec![Arg::Const(Const::new(0x1234, 2))];
    store.results = vec![Arg::Cell(Cell { r#ref: stored })];
    let mut returned = Op::new(0, OpCode::Operation(Operation::Return), "", vec![], vec![POINTER]);
    returned.kind = Kind::Return;
    returned.args = vec![Arg::Cell(Cell { r#ref: read })];
    let mut body = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![take, store, returned], vec![])]);
    body.sealed = true;
    body.stack_in_data = stack_in_data;
    body
}

fn based(addr: Addr, base: Value) -> MemRef {
    MemRef { base: Some(base), ..MemRef::new(Some(addr), 2) }
}

/// A store through `ds:[bx]` pointing at a DGROUP static was invisible to the static: two regions.
#[test]
fn test_a_near_pointer_to_a_dgroup_cell_reaches_that_cell() {
    let near = based(Addr::new(Space::Literal, 0), POINTER);
    let static_ = MemRef::new(Some(Addr { index: 3, ..Addr::new(Space::Segment, 4) }), 2);

    let body = _through_pointer(Arg::Const(Const::new(0x44, 2)), near, static_, false);
    let got = run(&body, &IndexMap::new(), &Memory::new(), None, 1_000_000, &BTreeMap::from([(3, 0x40)])).unwrap();
    assert_eq!(got.returned, vec![BigInt::from(0x1234)]);
}

/// BC's SS == DS: `[bp-4]` and `ds:[bx]` with bx = bp-4 are one byte, but were two regions.
#[test]
fn test_a_frame_slot_is_reached_through_a_near_pointer_when_the_stack_is_in_data() {
    let slot = MemRef { space: Some(Space::Frame), ..MemRef::new(Some(Addr::new(Space::Frame, -4)), 2) };
    let near = based(Addr::new(Space::Literal, 0), POINTER);

    let pointer = Arg::FrameAddress(FrameAddress { offset: -4, width: 2, extent: None });
    let body = _through_pointer(pointer, slot, near, true);
    let got = run(&body, &IndexMap::new(), &Memory::new(), None, 1_000_000, &BTreeMap::new()).unwrap();
    assert_eq!(got.returned, vec![BigInt::from(0x1234)]);
}
