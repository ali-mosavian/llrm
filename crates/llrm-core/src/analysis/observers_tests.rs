//! Port of tests/test_observers.py.

use std::collections::HashSet;
use std::rc::Rc;

use iced_x86::{Decoder, DecoderOptions, FlowControl, Instruction, Mnemonic, OpKind, Register};

use crate::support::hash::IndexMap;

use crate::analysis::avail;
use crate::frontends::bc::blocks::{self, Block};
use crate::wholeseg::Emission;
use crate::model::ir::Operation;
use crate::model::mir::{Arg, Cell, FrameAddress, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, Value};
use crate::objectfile::module::{Addr, Module, Space};
use crate::testing;

const NBODY: &str = concat!(env!("LLRM_ROOT"), "/tests/fixtures/bench/nbody-v-g3.obj");

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
    let found = testing::module(NBODY);
    let blocks = testing::blocks_of(&found);
    let bodies = testing::raised_from(&found, &blocks, None).values;
    (found, blocks, bodies)
}

fn private_in(body: &MirBody, found: &Rc<Module>, blocks: &Rc<Vec<Block>>) -> Box<dyn Fn(&MemRef) -> bool> {
    super::private(&std::rc::Rc::new(body.clone()), Some(found), Some(blocks)).unwrap().expect("a private test")
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

/// deltaX, deltaY, dist2, falloff and a product slot: five dead writes per pass.
#[test]
fn test_nbody_writes_no_scratch_variable_in_its_inner_loop() {
    let (result, states) = testing::emitted_mir(&testing::data(NBODY), "mir-widen", "main");
    assert_eq!(result.outcome, Emission::Lir, "{}", result.reason);
    let written: HashSet<(Space, i64)> = states[0]
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter_map(|op| avail::stored_cell(op).and_then(|cell| cell.addr.as_ref()))
        .map(|addr| (addr.space, addr.disp))
        .collect();
    let mut scratch: HashSet<(Space, i64)> = [0x76, 0x7A, 0x7E, 0x82].map(|disp| (Space::Segment, disp)).into();
    scratch.insert((Space::Frame, -0x18));
    assert!(written.is_disjoint(&scratch));
}

/// The rebuilt object's loops: the instructions from each backward branch's target to it.
fn _loops(obj: &str) -> Vec<Vec<Instruction>> {
    let result = testing::emitted_lir(obj);
    let found = testing::loaded_bytes(&result.data).unwrap();
    // A fresh OMF object deliberately has no BC module header for the legacy
    // mapper to recognize: its code segment is emitter-owned, so a linear
    // decode is exact.
    let insns: Vec<Instruction> = match blocks::code_map(&found) {
        Err(_) => Decoder::with_ip(16, &found.code, 0, DecoderOptions::NONE).into_iter().collect(),
        Ok(mapped) => blocks::partition(&found, &mapped)
            .iter()
            .flat_map(|block| block.insns.iter().map(|one| one.insn))
            .collect(),
    };
    insns
        .iter()
        .filter(|back| {
            back.flow_control() == FlowControl::ConditionalBranch && back.near_branch_target() < back.ip()
        })
        .map(|back| {
            insns.iter().filter(|one| back.near_branch_target() <= one.ip() && one.ip() <= back.ip()).copied().collect()
        })
        .collect()
}

/// The rebuilt nbody inner loop: the backward branch around the divide.
fn _nbody_inner_loop() -> Vec<Instruction> {
    _loops(NBODY).into_iter().find(|one| one.iter().any(|insn| insn.mnemonic() == Mnemonic::Idiv)).unwrap()
}

/// nbody's `other` was incremented and compared in a frame slot.
///
/// Exit-phi copies extended both accumulators across the branch and made the
/// allocator spill the hotter counter.
#[test]
fn test_nbody_counts_its_inner_loop_in_one_register() {
    let loop_ = _nbody_inner_loop();
    let branch_at =
        loop_.iter().rposition(|one| one.flow_control() == FlowControl::ConditionalBranch).unwrap();
    let compare_at = loop_[..branch_at].iter().rposition(|one| one.mnemonic() == Mnemonic::Cmp).unwrap();
    let compared = loop_[compare_at];
    assert_eq!(compared.op0_kind(), OpKind::Register);
    let counter = compared.op0_register();
    assert!(loop_[..compare_at].iter().any(|one| {
        one.mnemonic() == Mnemonic::Inc && one.op0_kind() == OpKind::Register && one.op0_register() == counter
    }));
}

/// PROCS printed TWICE= 3088: each call's DX store of r's high half went, and
/// only the low word of Twice& reached Report.
#[test]
fn test_a_long_handed_to_a_sub_keeps_both_halves_stored() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        let result = testing::emitted_lir(format!("{}/tests/fixtures/omf/procs-{}.obj", env!("LLRM_ROOT"), tag.to_lowercase()));
        let stores = testing::instructions(&result.data)
            .into_iter()
            .filter(|one| {
                one.mnemonic() == Mnemonic::Mov
                    && one.op0_kind() == OpKind::Memory
                    && one.memory_base() == Register::None
                    && one.op1_kind() == OpKind::Register
            })
            .filter(|one| one.op1_register() == Register::DX)
            .count();
        assert_eq!(stores, 3, "{tag}");
    }
}
