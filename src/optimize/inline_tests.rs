//! Ports of `tests/test_inline.py`.
//!
//! Skipped: `test_small_private_pure_helpers_inline_in_mir` and
//! `test_tiny_private_leaf_inlines_at_two_call_sites` (need the ported cfront
//! optimizer, which waits on `optimize/transform.py`).

use std::collections::BTreeSet;

use crate::support::hash::IndexMap;

use super::*;
use crate::model::ir::Operation;
use crate::objectfile::module::{Addr, Space};

fn value(id: u32, at: i64, variable: u32) -> Value {
    Value { id, at, flags: false, variable, version: 1 }
}

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

fn _copy(at: i64, value: Value, number: i64) -> Op {
    let mut made = op(at, Operation::Nothing, "", Kind::Copy);
    made.defines = vec![value];
    made.args = vec![Arg::Const(Const::new(number, 2))];
    made.results = vec![Arg::Held(Held { value, width: 2 })];
    made
}

fn returning(at: i64, returned: Value) -> Op {
    let mut made = op(at, Operation::Nothing, "", Kind::Return);
    made.uses = vec![returned];
    made.args = vec![Arg::Held(Held { value: returned, width: 2 })];
    made
}

fn _leaf() -> MirBody {
    let value = value(1, 1, 1);
    sealed(1, vec![MirBlock::new(1, vec![], vec![_copy(1, value, 37), returning(2, value)], vec![])])
}

fn _caller(use_clobber: bool) -> (MirBody, Value) {
    let result = value(10, 2, 10);
    let clobber = value(11, 2, 11);
    let mut call = op(2, Operation::Nothing, "", Kind::Call);
    call.defines = vec![result, clobber];
    call.results = vec![Arg::Held(Held { value: result, width: 2 }), Arg::Held(Held { value: clobber, width: 2 })];
    let mut ops = vec![call];
    if use_clobber {
        let observed = value(12, 3, 12);
        let mut copy = op(3, Operation::Nothing, "", Kind::Copy);
        copy.defines = vec![observed];
        copy.uses = vec![clobber];
        copy.args = vec![Arg::Held(Held { value: clobber, width: 2 })];
        copy.results = vec![Arg::Held(Held { value: observed, width: 2 })];
        ops.push(copy);
    }
    let joined = Value { id: 13, at: 4, flags: false, variable: 10, version: 2 };
    let mut phi = Phi::new(joined);
    phi.incoming.insert(1, result);
    (
        sealed(
            1,
            vec![MirBlock::new(1, vec![], ops, vec![4]), MirBlock::new(4, vec![phi], vec![returning(4, joined)], vec![])],
        ),
        result,
    )
}

fn leaf_available(leaf: MirBody, parameters: Vec<MemRef>) -> IndexMap<String, Candidate> {
    IndexMap::from_iter([("leaf".to_owned(), Candidate { body: leaf, parameters })])
}

#[test]
fn test_inline_splices_return_before_the_original_successor_phi() {
    let (body, result) = _caller(false);
    let made = expanded(
        &body,
        &IndexMap::from_iter([(2, "leaf".to_owned())]),
        &IndexMap::from_iter([(2, BTreeSet::new())]),
        &leaf_available(_leaf(), vec![]),
        None,
    )
    .unwrap();

    assert_ne!(made, body);
    assert!(!made.blocks.iter().flat_map(|block| &block.ops).any(|op| op.kind == Kind::Call));
    let successor = made.block(4).unwrap();
    assert_ne!(successor.phis[0].incoming.keys().copied().collect::<BTreeSet<_>>(), BTreeSet::from([1]));
    assert_eq!(successor.phis[0].incoming.values().copied().collect::<BTreeSet<_>>(), BTreeSet::from([result]));
    assert_eq!(mir::verify(&made), Vec::<String>::new());
}

#[test]
fn test_inline_refuses_a_live_unmodelled_call_result() {
    let (body, _) = _caller(true);
    assert_eq!(
        expanded(
            &body,
            &IndexMap::from_iter([(2, "leaf".to_owned())]),
            &IndexMap::from_iter([(2, BTreeSet::new())]),
            &leaf_available(_leaf(), vec![]),
            None,
        )
        .unwrap(),
        body
    );
}

#[test]
fn test_inline_materializes_an_actual_whose_id_is_a_callee_substitution_key() {
    let mut parameter = MemRef::new(Some(Addr::new(Space::Frame, 4)), 2);
    parameter.space = Some(Space::Frame);
    let formal = value(1, 1, 1);
    let colliding = value(2, 2, 2);
    let mut load = op(1, Operation::Move, "mov", Kind::Load);
    load.defines = vec![formal];
    load.loads = vec![parameter.clone()];
    load.args = vec![Arg::Cell(mir::Cell { r#ref: parameter.clone() })];
    load.results = vec![Arg::Held(Held { value: formal, width: 2 })];
    let unrelated = _copy(2, colliding, 99);
    let leaf = sealed(1, vec![MirBlock::new(1, vec![], vec![load, unrelated, returning(3, formal)], vec![])]);

    let actual = Value { id: 2, at: 1, flags: false, variable: 20, version: 1 };
    let result = Value { id: 3, at: 3, flags: false, variable: 30, version: 1 };
    let mut argument = op(2, Operation::Nothing, "", Kind::Arg);
    argument.uses = vec![actual];
    argument.args = vec![Arg::Held(Held { value: actual, width: 2 })];
    let mut call = op(3, Operation::Nothing, "", Kind::Call);
    call.defines = vec![result];
    call.results = vec![Arg::Held(Held { value: result, width: 2 })];
    let caller = sealed(1, vec![MirBlock::new(1, vec![], vec![_copy(1, actual, 7), argument, call], vec![])]);

    let made = expanded(
        &caller,
        &IndexMap::from_iter([(3, "leaf".to_owned())]),
        &IndexMap::from_iter([(3, BTreeSet::from([2]))]),
        &leaf_available(leaf, vec![parameter]),
        None,
    )
    .unwrap();

    let returned_copies = made
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| op.kind == Kind::Copy && op.defines.contains(&result))
        .collect::<Vec<_>>();
    assert_eq!(returned_copies.len(), 1);
    let Arg::Held(source) = &returned_copies[0].args[0] else { panic!("expected a held source") };
    assert_ne!(source.value, colliding);
    let definitions = made
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| op.defines.contains(&source.value))
        .collect::<Vec<_>>();
    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].args, vec![Arg::Held(Held { value: actual, width: 2 })]);
    assert_eq!(mir::verify(&made), Vec::<String>::new());
}

#[test]
fn test_inline_policy_refuses_repeated_work_without_a_call_cost() {
    let leaf = _leaf();
    let leaf_set = BTreeSet::from(["leaf".to_owned()]);
    assert_eq!(
        candidates(
            &IndexMap::from_iter([("leaf".to_owned(), leaf)]),
            &IndexMap::from_iter([("leaf".to_owned(), vec![])]),
            &Counter::from_iter([("leaf".to_owned(), 2)]),
            &leaf_set,
            &leaf_set,
            0,
        ),
        IndexMap::default()
    );
}
