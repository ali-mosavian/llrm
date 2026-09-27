//! Port of `tests/test_constant_cycles.py`.

use std::rc::Rc;
use super::super::consts::{Known, known};
use crate::model::ir::Operation;
use crate::model::mir::{Arg, Const, Held, Kind, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi, Value};

fn body_with_cycle(step: i64, external: bool) -> (MirBody, Value, Value) {
    let (start, joined, carried, incoming) = (Value::new(1, 0), Value::new(2, 0), Value::new(3, 0), Value::new(4, 0));
    let mut seed = Op::new(0, OpCode::Operation(Operation::Move), "mov", vec![start], vec![]);
    seed.kind = Kind::Copy;
    seed.args = vec![Arg::Const(Const::new(7, 4))];
    seed.results = vec![Arg::Held(Held { value: start, width: 4 })];
    let source = if external { incoming } else { joined };
    let mut update = Op::new(10, OpCode::Operation(Operation::Binary), "add", vec![carried], vec![source]);
    update.kind = Kind::Add;
    update.args = vec![Arg::Held(Held { value: source, width: 4 }), Arg::Const(Const::new(step, 4))];
    update.results = vec![Arg::Held(Held { value: carried, width: 4 })];
    let incoming = OrderedMap::from_iter([(0, start), (10, carried)]);
    let body = MirBody::new(
        0,
        vec![
            MirBlock::new(0, vec![], vec![seed], vec![10]),
            MirBlock::new(10, vec![Phi { result: joined, incoming }], vec![update], vec![10, 20]),
            MirBlock::new(20, vec![], vec![], vec![]),
        ],
    );
    (body, joined, carried)
}

#[test]
fn test_unchanged_loop_value_is_constant_through_its_backedge() {
    let (body, joined, carried) = body_with_cycle(0, false);
    let facts = known(&Rc::new(MirBody::clone(&body)), None, None, None, None);
    assert_eq!(facts[&joined], Known::new(7, 4));
    assert_eq!(facts[&carried], Known::new(7, 4));
}

#[test]
fn test_changed_or_runtime_backedge_is_not_the_initial_constant() {
    for (step, external) in [(1, false), (0, true)] {
        let (body, joined, carried) = body_with_cycle(step, external);
        let facts = known(&Rc::new(MirBody::clone(&body)), None, None, None, None);
        assert!(!facts.contains_key(&joined) && !facts.contains_key(&carried));
    }
}

#[test]
fn test_unanchored_cycle_does_not_invent_a_constant() {
    let (mut body, joined, carried) = body_with_cycle(0, false);
    body.blocks[1].phis = vec![Phi {
        result: joined,
        incoming: OrderedMap::from_iter([(10, carried)]),
    }];
    assert!(!known(&Rc::new(MirBody::clone(&body)), None, None, None, None).contains_key(&joined));
}

#[test]
fn test_cyclic_propagation_does_not_widen_a_known_word() {
    let (mut body, joined, carried) = body_with_cycle(0, false);
    let seed = &mut body.blocks[0].ops[0];
    seed.args = vec![Arg::Const(Const::new(7, 2))];
    seed.results = vec![Arg::Held(Held { value: seed.defines[0], width: 2 })];
    let facts = known(&Rc::new(MirBody::clone(&body)), None, None, None, None);
    assert!(!facts.contains_key(&joined) && !facts.contains_key(&carried));
}

#[test]
fn test_block_order_does_not_change_cyclic_facts() {
    let (body, _, _) = body_with_cycle(0, false);
    let mut reordered = body.clone();
    reordered.blocks.reverse();
    assert_eq!(known(&Rc::new(MirBody::clone(&body)), None, None, None, None), known(&Rc::new(MirBody::clone(&reordered)), None, None, None, None));
}
