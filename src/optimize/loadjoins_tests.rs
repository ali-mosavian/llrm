//! Port of tests/test_memory_joins.py and tests/test_load_pre.py.
//!
//! Waiting for `wholeseg.emitted`:
//! `test_arrphi_keeps_the_stored_element_value_across_each_join`,
//! `test_emitted_memphi_reloads_read_destination_but_not_array_values`,
//! `test_ldpre_true_arm_skips_the_remaining_memory_read`,
//! `test_ldcrit_load_runs_only_on_the_missing_conditional_edge`.

use super::*;
use crate::model::mir::Const;
use crate::objectfile::module::Addr;

fn op(at: i64, code: Operation, defines: Vec<Value>, uses: Vec<Value>, kind: Kind) -> Op {
    let mut op = Op::new(at, OpCode::Operation(code), "", defines, uses);
    op.kind = kind;
    op
}

fn block(at: i64, phis: Vec<Phi>, ops: Vec<Op>, succ: Vec<i64>) -> MirBlock {
    MirBlock::new(at, phis, ops, succ)
}

fn pointer_ref(base: Value, width: u32) -> MemRef {
    MemRef { base: Some(base), pointer: true, ..MemRef::new(None, width) }
}

fn phi(result: Value, incoming: &[(i64, Value)]) -> Phi {
    Phi { result, incoming: incoming.iter().copied().collect() }
}

fn reused_(body: &MirBody, insert: bool) -> MirBody {
    MirBody::clone(&reused(&std::rc::Rc::new(body.clone()), None, insert).expect("reused"))
}

// ---- tests/test_memory_joins.py ----

fn diamond() -> MirBody {
    let [pointer, left, right, loaded] = [(1, 0), (2, 10), (3, 20), (4, 30)].map(|(index, at)| Value::new(index, at));
    let cell = pointer_ref(pointer, 4);
    let store = |at: i64, value: Value| -> Vec<Op> {
        let mut source = op(at, Operation::Move, vec![value], vec![], Kind::Copy);
        source.args = vec![Arg::Const(Const::new(at, 4))];
        source.results = vec![Arg::Held(Held { value, width: 4 })];
        let mut write = op(at + 1, Operation::Move, vec![], vec![value, pointer], Kind::Store);
        write.args = vec![Arg::Held(Held { value, width: 4 })];
        write.results = vec![Arg::Cell(Cell { r#ref: cell.clone() })];
        write.stores = vec![cell.clone()];
        vec![source, write]
    };
    let mut read = op(30, Operation::Move, vec![loaded], vec![pointer], Kind::Load);
    read.args = vec![Arg::Cell(Cell { r#ref: cell.clone() })];
    read.results = vec![Arg::Held(Held { value: loaded, width: 4 })];
    read.loads = vec![cell.clone()];
    MirBody::new(
        0,
        vec![
            block(0, vec![], vec![], vec![10, 20]),
            block(10, vec![], store(10, left), vec![30]),
            block(20, vec![], store(20, right), vec![30]),
            block(30, vec![], vec![read], vec![]),
        ],
    )
}

fn pointer_diamond() -> MirBody {
    let mut body = diamond();
    let [left, right, joined] = [(5, 10), (6, 20), (7, 30)].map(|(index, at)| Value::new(index, at));
    for (block, pointer) in body.blocks[1..3].iter_mut().zip([left, right]) {
        let write = block.ops.last_mut().expect("a write");
        let r#ref = MemRef { base: Some(pointer), ..write.stores[0].clone() };
        write.stores = vec![r#ref.clone()];
        write.results = vec![Arg::Cell(Cell { r#ref })];
    }
    let join = body.blocks.last_mut().expect("a join");
    let read = &mut join.ops[0];
    let r#ref = MemRef { base: Some(joined), ..read.loads[0].clone() };
    read.loads = vec![r#ref.clone()];
    read.args = vec![Arg::Cell(Cell { r#ref })];
    join.phis = vec![phi(joined, &[(10, left), (20, right)])];
    body
}

/// ARRPHI still reloaded both elements after sharing their addresses across branches.
#[test]
fn test_pointer_phi_selects_the_matching_store_on_each_edge() {
    let after = reused_(&pointer_diamond(), false);
    assert!(after.blocks.last().unwrap().ops.last().unwrap().loads.is_empty());
}

#[test]
fn test_crossed_pointer_phi_does_not_reuse_the_other_branches_store() {
    let mut body = pointer_diamond();
    let join = body.blocks.last_mut().unwrap();
    let crossed = phi(join.phis[0].result, &[(10, join.phis[0].incoming.get(&20).copied().unwrap()), (20, join.phis[0].incoming.get(&10).copied().unwrap())]);
    join.phis = vec![crossed];
    assert_eq!(reused_(&body, false), body);
}

#[test]
fn test_join_prefix_write_through_the_pointer_phi_blocks_reuse() {
    let mut body = pointer_diamond();
    let join = body.blocks.last_mut().unwrap();
    let cell = join.ops[0].loads[0].clone();
    let mut write = op(30, Operation::Move, vec![], vec![cell.base.unwrap()], Kind::Store);
    write.args = vec![Arg::Const(Const::new(99, 4))];
    write.results = vec![Arg::Cell(Cell { r#ref: cell.clone() })];
    write.stores = vec![cell];
    join.ops.insert(0, write);
    assert_eq!(reused_(&body, false), body);
}

#[test]
fn test_join_load_uses_each_predecessors_stored_value() {
    for reverse in [false, true] {
        let mut body = diamond();
        if reverse {
            body.blocks.reverse();
        }
        let after = reused_(&body, false);
        let join = after.blocks.iter().find(|block| block.at == 30).unwrap();
        assert!(join.ops[0].loads.is_empty());
        let [phi] = &join.phis[..] else { panic!("one phi") };
        assert_eq!(join.ops[0].args, vec![Arg::Held(Held { value: phi.result, width: 4 })]);
        let expected: Vec<(i64, Value)> = body
            .blocks
            .iter()
            .filter(|block| [10, 20].contains(&block.at))
            .map(|block| (block.at, block.ops[0].defines[0]))
            .collect();
        let mut got: Vec<(i64, Value)> = phi.incoming.iter().map(|(at, value)| (*at, *value)).collect();
        got.sort();
        let mut expected = expected;
        expected.sort();
        assert_eq!(got, expected);
        assert_eq!(reused_(&after, false), after);
    }
}

#[test]
fn test_observable_call_or_exception_invalidates_the_join_value() {
    for r#where in [10, 20, 30] {
        for kind in [Kind::Call, Kind::Fcheck] {
            let mut body = diamond();
            let call = op(r#where, Operation::Call, vec![], vec![], kind);
            for block in &mut body.blocks {
                if block.at == r#where {
                    if r#where == 30 {
                        block.ops.insert(0, call.clone());
                    } else {
                        block.ops.push(call.clone());
                    }
                }
            }
            assert_eq!(reused_(&body, false), body);
        }
    }
}

#[test]
fn test_a_missing_edge_provider_keeps_the_load() {
    let mut body = diamond();
    for block in &mut body.blocks {
        if block.at == 20 {
            block.ops = vec![];
        }
    }
    assert_eq!(reused_(&body, false), body);
}

#[test]
fn test_a_partial_overwrite_invalidates_the_whole_value() {
    for r#where in [10, 30] {
        let mut body = diamond();
        let cell = body.blocks.last().unwrap().ops[0].loads[0].clone();
        let narrow = MemRef { width: 1, ..cell.clone() };
        let mut write = op(r#where, Operation::Move, vec![], vec![cell.base.unwrap()], Kind::Store);
        write.args = vec![Arg::Const(Const::new(99, 1))];
        write.results = vec![Arg::Cell(Cell { r#ref: narrow.clone() })];
        write.stores = vec![narrow];
        for block in &mut body.blocks {
            if block.at == r#where {
                if r#where == 30 {
                    block.ops.insert(0, write.clone());
                } else {
                    block.ops.push(write.clone());
                }
            }
        }
        assert_eq!(reused_(&body, false), body);
    }
}

#[test]
fn test_predecessor_loads_can_supply_the_join_without_stores() {
    let mut body = diamond();
    let load = body.blocks.last().unwrap().ops[0].clone();
    for block in &mut body.blocks {
        if [10, 20].contains(&block.at) {
            let Arg::Held(value) = block.ops[0].results[0] else { panic!("held") };
            let mut read = load.clone();
            read.at = block.at;
            read.defines = vec![value.value];
            read.results = vec![Arg::Held(value)];
            block.ops = vec![read];
        }
    }
    assert!(reused_(&body, false).blocks.last().unwrap().ops[0].loads.is_empty());
}

// ---- tests/test_load_pre.py ----

fn pre_diamond() -> MirBody {
    let cell = MemRef::new(Some(Addr { index: 5, ..Addr::new(Space::Segment, 6) }), 2);
    let (before, after) = (Value::new(1, 10), Value::new(2, 30));
    let mut load = op(10, Operation::Move, vec![before], vec![], Kind::Load);
    load.args = vec![Arg::Cell(Cell { r#ref: cell.clone() })];
    load.results = vec![Arg::Held(Held { value: before, width: 2 })];
    load.loads = vec![cell];
    let mut repeated = load.clone();
    repeated.at = 30;
    repeated.defines = vec![after];
    repeated.results = vec![Arg::Held(Held { value: after, width: 2 })];
    MirBody::new(
        0,
        vec![
            block(0, vec![], vec![], vec![10, 20]),
            block(10, vec![], vec![load], vec![30]),
            block(20, vec![], vec![], vec![30]),
            block(30, vec![], vec![repeated], vec![]),
        ],
    )
}

#[test]
fn test_missing_path_reads_once_and_existing_path_reuses_value() {
    let body = pre_diamond();
    let result = reused_(&body, true);
    assert_eq!(result.block(10).unwrap().ops, body.block(10).unwrap().ops);
    let [inserted] = &result.block(20).unwrap().ops[..] else { panic!("one insertion") };
    assert_eq!(inserted.loads, body.block(30).unwrap().ops[0].loads);
    assert!(inserted.inserted() && !inserted.source_backed);
    assert!(result.block(30).unwrap().ops[0].loads.is_empty());
    let [phi] = &result.block(30).unwrap().phis[..] else { panic!("one phi") };
    let got: Vec<(i64, Value)> = phi.incoming.iter().map(|(at, value)| (*at, *value)).collect();
    assert_eq!(got, vec![(10, body.block(10).unwrap().ops[0].defines[0]), (20, inserted.defines[0])]);
    assert_eq!(reused_(&result, true), result);
}

#[test]
fn test_load_insertion_cannot_speculate_or_cross_observable_operations() {
    for guard in ["critical", "call", "checkpoint", "store", "division", "no_provider"] {
        let mut body = pre_diamond();
        match guard {
            "critical" => {
                for block in &mut body.blocks {
                    if block.at == 20 {
                        block.succ = vec![30, 40];
                    }
                }
                body.blocks.push(block(40, vec![], vec![], vec![]));
            }
            "no_provider" => {
                for block in &mut body.blocks {
                    if block.at == 10 {
                        block.ops = vec![];
                    }
                }
            }
            _ => {
                let kind = match guard {
                    "call" => Kind::Call,
                    "checkpoint" => Kind::Fcheck,
                    "store" => Kind::Store,
                    _ => Kind::Div,
                };
                let effect = op(29, Operation::Nothing, vec![], vec![], kind);
                body.blocks.last_mut().unwrap().ops.insert(0, effect);
            }
        }
        assert_eq!(reused_(&body, true), body, "{guard}");
    }
}

#[test]
fn test_inserted_address_uses_the_missing_edges_pointer() {
    let mut body = pre_diamond();
    let [left, right, joined] = [(5, 10), (6, 20), (7, 30)].map(|(index, at)| Value::new(index, at));
    for (block, pointer) in body.blocks[1..3].iter_mut().zip([left, right]) {
        let mut define = op(block.at, Operation::Move, vec![pointer], vec![], Kind::Copy);
        define.args = vec![Arg::Const(Const::new(block.at, 4))];
        define.results = vec![Arg::Held(Held { value: pointer, width: 4 })];
        let ops = block.ops.iter().map(|one| {
            let mut one = one.clone();
            one.uses = vec![pointer];
            one.loads = vec![pointer_ref(pointer, 2)];
            one.args = vec![Arg::Cell(Cell { r#ref: pointer_ref(pointer, 2) })];
            one
        });
        block.ops = std::iter::once(define).chain(ops).collect();
    }
    let join = body.blocks.last_mut().unwrap();
    let r#ref = pointer_ref(joined, 2);
    join.phis = vec![phi(joined, &[(10, left), (20, right)])];
    let read = &mut join.ops[0];
    read.uses = vec![joined];
    read.loads = vec![r#ref.clone()];
    read.args = vec![Arg::Cell(Cell { r#ref })];
    let result = reused_(&body, true);
    let last = result.block(20).unwrap().ops.last().unwrap();
    assert_eq!(last.loads[0].base, Some(right));
    assert_eq!(last.uses, vec![right]);
    assert!(result.block(30).unwrap().ops[0].loads.is_empty());
}

/// The unrelated arm must not acquire a read that could fault or observe memory.
#[test]
fn test_missing_explicit_critical_edge_gets_its_own_load_block() {
    let mut body = pre_diamond();
    let mut condition = op(20, Operation::Branch, vec![], vec![], Kind::Branch);
    condition.target = Some(30);
    condition.test = Some(Kind::Eq);
    for block in &mut body.blocks {
        if block.at == 20 {
            block.ops = vec![condition.clone()];
            block.succ = vec![30, 40];
        }
    }
    body.blocks.push(block(40, vec![], vec![], vec![]));
    let result = reused_(&body, true);
    assert!(result.block(30).unwrap().ops[0].loads.is_empty());
    let parent = result.block(20).unwrap();
    assert!(parent.ops.len() == 1 && parent.succ.contains(&40));
    let bridge = result.block(parent.ops.last().unwrap().target.unwrap()).unwrap();
    assert!(!body.blocks.iter().any(|block| block.at == bridge.at));
    assert_eq!(bridge.ops[0].loads, body.block(30).unwrap().ops[0].loads);
    assert!(bridge.ops.last().unwrap().kind == Kind::Jump && bridge.ops.last().unwrap().target == Some(30));
    assert!(result.block(30).unwrap().phis[0].incoming.contains_key(&bridge.at));
    assert!(!result.block(30).unwrap().phis[0].incoming.contains_key(&20));
    assert!(bridge.ops.iter().all(Op::inserted));
    assert_eq!(reused_(&result, true), result);
}
