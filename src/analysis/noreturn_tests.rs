//! Ports of the `noreturn` tests in `tests/test_noreturn.py` and
//! `tests/test_sccp.py`.
//!
//! Skipped: `test_qrender_main_spill_uses_shutdown_control_proof` (needs the
//! object corpus, `runtime.for_module`, `lower`, `allocate`, `frame` and
//! `prologue` on the BASIC path, not yet ported).

use std::collections::BTreeSet;

use crate::support::hash::IndexMap;

use super::{after_terminal_calls, inferred};
use crate::model::ir::Operation;
use crate::model::mir::{Kind, MirBlock, MirBody, Op, OpCode};

fn op(at: i64, operation: Operation, name: &str, kind: Kind) -> Op {
    let mut made = Op::new(at, OpCode::Operation(operation), name, vec![], vec![]);
    made.kind = kind;
    made
}

fn sealed(entry: i64, blocks: Vec<MirBlock>) -> MirBody {
    let mut body = MirBody::new(entry, blocks);
    body.sealed = true;
    body
}

#[test]
fn test_closed_local_terminal_scc_is_noreturn() {
    let call_b = op(2, Operation::Nothing, "", Kind::Call);
    let call_a = op(4, Operation::Nothing, "", Kind::Call);
    let first = sealed(0x30, vec![MirBlock::new(0x30, vec![], vec![call_b], vec![])]);
    let second = sealed(0x40, vec![MirBlock::new(0x40, vec![], vec![call_a], vec![])]);

    assert_eq!(
        inferred(
            &IndexMap::from_iter([(0x30, first), (0x40, second)]),
            &IndexMap::from_iter([(2, 0x40), (4, 0x30)]),
            &BTreeSet::new(),
        ),
        BTreeSet::from([0x30, 0x40])
    );
}

#[test]
fn test_terminal_call_inerts_newly_unreachable_successor() {
    let terminal = op(2, Operation::Nothing, "", Kind::Call);
    let store = op(10, Operation::Move, "mov", Kind::Store);
    let body = sealed(
        0,
        vec![
            MirBlock::new(0, vec![], vec![terminal], vec![10]),
            MirBlock::new(10, vec![], vec![store], vec![]),
        ],
    );

    let trimmed = after_terminal_calls(&body, &BTreeSet::from([2]));

    assert!(trimmed.block(0).unwrap().succ.is_empty());
    let orphan = trimmed.block(10).unwrap();
    assert!(orphan.succ.is_empty());
    assert!(orphan.ops.iter().all(|op| op.kind == Kind::Nothing && op.stores.is_empty()));
}

#[test]
fn test_terminal_call_inerts_its_same_block_source_tail() {
    let terminal = op(2, Operation::Nothing, "", Kind::Call);
    let mut dead_jump = op(3, Operation::Branch, "jmp", Kind::Branch);
    dead_jump.target = Some(20);
    dead_jump.absorbed = vec![3];
    let body = sealed(
        0,
        vec![
            MirBlock::new(0, vec![], vec![terminal, dead_jump], vec![20]),
            MirBlock::new(20, vec![], vec![], vec![]),
        ],
    );

    let trimmed = after_terminal_calls(&body, &BTreeSet::from([2]));

    let owner = trimmed.block(0).unwrap().ops.last().unwrap();
    assert_eq!(owner.at, 3);
    assert_eq!(owner.absorbed, vec![3]);
    assert_eq!(owner.kind, Kind::Nothing);
    assert_eq!(owner.target, None);
    assert!(trimmed.block(0).unwrap().succ.is_empty());
}
