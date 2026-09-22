//! Port of tests/test_observers.py.
//!
//! Skipped, needing `wholeseg.emitted`:
//! `test_nbody_writes_no_scratch_variable_in_its_inner_loop`,
//! `test_nbody_counts_its_inner_loop_in_one_register`,
//! `test_a_long_handed_to_a_sub_keeps_both_halves_stored`.

use std::rc::Rc;

use crate::support::hash::IndexMap;

use crate::analysis::avail;
use crate::frontend::blocks::Block;
use crate::model::ir::Operation;
use crate::model::mir::{Arg, Cell, FrameAddress, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, Value};
use crate::objectfile::module::{Addr, Module, Space};
use crate::optimize::testcorpus;

const NBODY: &str = "fixtures/bench/nbody-v-g3.obj";

fn x() -> MemRef {
    MemRef::new(Some(Addr { index: 5, ..Addr::new(Space::Segment, 0x76) }), 4)
}

fn _store(at: i64, cell: MemRef) -> Op {
    let value = Value::new(u32::try_from(at + 1).expect("small"), 0);
    let mut op = Op::new(at, OpCode::Operation(Operation::Move), "mov", vec![], vec![value]);
    op.kind = Kind::Store;
    op.args = vec![Arg::Held(Held { value, width: cell.width })];
    op.results = vec![Arg::Cell(Cell { r#ref: cell.clone() })];
    op.stores = vec![cell];
    op
}

fn _load(at: i64, cell: MemRef) -> Op {
    let value = Value::new(u32::try_from(at + 1).expect("small"), 0);
    let mut op = Op::new(at, OpCode::Operation(Operation::Move), "mov", vec![value], vec![]);
    op.kind = Kind::Load;
    op.args = vec![Arg::Cell(Cell { r#ref: cell.clone() })];
    op.results = vec![Arg::Held(Held { value, width: cell.width })];
    op.loads = vec![cell];
    op
}

fn _call(at: i64) -> Op {
    let mut op = Op::new(at, OpCode::Operation(Operation::Call), "call", vec![], vec![]);
    op.kind = Kind::Call;
    op
}

fn _dead(body: &MirBody, private: Option<&dyn Fn(&MemRef) -> bool>) -> Vec<i64> {
    avail::dead_stores(body, None, &IndexMap::default(), private, None, true).iter().map(|op| op.at).collect()
}

fn _everything(_ref: &MemRef) -> bool {
    true
}

/// nbody wrote deltaX on every inner pass: the back edge vetoed the proof.
#[test]
fn test_a_store_no_iteration_reads_is_dead_inside_its_loop() {
    let body = MirBody::new(
        0,
        vec![
            MirBlock::new(0, vec![], vec![], vec![10]),
            MirBlock::new(10, vec![], vec![], vec![12, 14]),
            MirBlock::new(12, vec![], vec![_store(12, x())], vec![14]),
            MirBlock::new(14, vec![], vec![], vec![10, 20]),
            MirBlock::new(20, vec![], vec![], vec![]),
        ],
    );
    assert_eq!(_dead(&body, Some(&_everything)), vec![12]);
}

#[test]
fn test_a_store_the_next_iteration_reads_stays() {
    let body = MirBody::new(
        0,
        vec![
            MirBlock::new(0, vec![], vec![], vec![10]),
            MirBlock::new(10, vec![], vec![_load(10, x()), _store(12, x())], vec![10, 20]),
            MirBlock::new(20, vec![], vec![], vec![]),
        ],
    );
    assert_eq!(_dead(&body, Some(&_everything)), Vec::<i64>::new());
}

#[test]
fn test_a_call_cannot_read_a_private_cell() {
    let body = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![_store(0, x()), _call(2)], vec![])]);
    assert_eq!(_dead(&body, Some(&_everything)), vec![0]);
    assert_eq!(_dead(&body, None), Vec::<i64>::new());
}

#[test]
fn test_an_unresolved_address_cannot_read_a_private_cell_but_its_name_can() {
    let unknown = MemRef::new(None, 2);
    let blind = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![_store(0, x()), _load(2, unknown)], vec![])]);
    let named = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![_store(0, x()), _load(2, x())], vec![])]);
    assert_eq!(_dead(&blind, Some(&_everything)), vec![0]);
    assert_eq!(_dead(&named, Some(&_everything)), Vec::<i64>::new());
}

fn nbody() -> (Rc<Module>, Rc<Vec<Block>>, Vec<(String, Rc<MirBody>)>) {
    let found = testcorpus::loaded(NBODY);
    let blocks = testcorpus::partitioned(&found);
    let bodies = testcorpus::raised(&found, &blocks, None).values;
    (found, blocks, bodies)
}

fn private_in(body: &MirBody, found: &Rc<Module>, blocks: &Rc<Vec<Block>>) -> Box<dyn Fn(&MemRef) -> bool> {
    super::private(body, Some(found), Some(blocks)).unwrap().expect("a private test")
}

#[test]
fn test_a_taken_frame_address_makes_no_frame_cell_private() {
    let (found, blocks, bodies) = nbody();
    let main = &bodies[0].1;
    let slot = MemRef::new(Some(Addr::new(Space::Frame, -0x18)), 4);
    assert!(private_in(main, &found, &blocks)(&slot));
    let mut push = Op::new(0, OpCode::Operation(Operation::Push), "push", vec![], vec![]);
    push.kind = Kind::Arg;
    push.args = vec![Arg::FrameAddress(FrameAddress::new(-0x18, 2))];
    let mut taken = vec![MirBlock::new(main.entry, vec![], vec![push], vec![])];
    taken.extend(main.blocks[1..].iter().cloned());
    assert!(!private_in(&MirBody::new(main.entry, taken), &found, &blocks)(&slot));
}

#[test]
fn test_only_the_main_body_owns_a_variable_nothing_else_names() {
    let (found, blocks, bodies) = nbody();
    let named = |name: &str| bodies.iter().find(|(one, _)| one == name).expect(name).1.clone();
    let main = private_in(&named("main (main)"), &found, &blocks);
    let procedure = private_in(&named("procedure PITSNAP"), &found, &blocks);
    // its address goes to PitSnap
    let t_start = MemRef::new(Some(Addr { index: 5, ..Addr::new(Space::Segment, 0x9E) }), 4);
    assert!(main(&x()));
    assert!(!main(&t_start));
    assert!(!procedure(&x()));
}
