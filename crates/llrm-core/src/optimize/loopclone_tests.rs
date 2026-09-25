//! Port of `tests/test_loopclone.py`. `is None` becomes `Ok(None)`.

use std::collections::{BTreeMap, BTreeSet};

use super::peeled;
use crate::analysis::loops;
use crate::model::floating::{Format, Precision, Rounding, Semantics};
use crate::model::ir::Operation;
use crate::model::memory::{Identity, MemoryKind, MemoryObject, Provenance};
use crate::model::mir::{Arg, Held, IntegerRange, Kind, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi, Value};

fn diamond() -> (MirBody, Vec<Value>) {
    let values = (1..8).map(|index| Value { id: index, at: i64::from(index), flags: false, variable: index, version: 1 }).collect::<Vec<_>>();
    let (seed, carried, left, right, selected, stepped, answer) =
        (values[0], values[1], values[2], values[3], values[4], values[5], values[6]);

    let copy = |at: i64, result: Value, source: Value| {
        let mut op = Op::new(at, OpCode::Operation(Operation::Move), "", vec![result], vec![source]);
        op.kind = Kind::Copy;
        op.args = vec![Arg::Held(Held { value: source, width: 2 })];
        op.results = vec![Arg::Held(Held { value: result, width: 2 })];
        op
    };
    let branch = |at: i64, target: i64| {
        let mut op = Op::new(at, OpCode::Operation(Operation::Branch), "", vec![], vec![carried]);
        op.kind = Kind::Branch;
        op.target = Some(target);
        op.args = vec![Arg::Held(Held { value: carried, width: 2 })];
        op
    };
    let phi = |result: Value, pairs: &[(i64, Value)]| Phi { result, incoming: pairs.iter().copied().collect() };

    let body = MirBody::new(
        0,
        vec![
            MirBlock::new(0, vec![], vec![], vec![1]),
            MirBlock::new(1, vec![phi(carried, &[(0, seed), (5, stepped)])], vec![branch(10, 2)], vec![2, 6]),
            MirBlock::new(2, vec![], vec![branch(20, 3)], vec![3, 4]),
            MirBlock::new(3, vec![], vec![copy(30, left, carried), branch(31, 6)], vec![5, 6]),
            MirBlock::new(4, vec![], vec![copy(40, right, carried)], vec![5]),
            MirBlock::new(5, vec![phi(selected, &[(3, left), (4, right)])], vec![copy(50, stepped, selected)], vec![1]),
            MirBlock::new(6, vec![phi(answer, &[(1, carried), (3, left)])], vec![], vec![]),
        ],
    );
    (body, values)
}

fn only_loop(body: &MirBody) -> loops::Loop {
    let found = loops::loops(&body.blocks, Some(body.entry));
    assert_eq!(found.len(), 1);
    found.into_iter().next().unwrap()
}

fn definitions(block: &MirBlock) -> impl Iterator<Item = Value> + '_ {
    block.phis.iter().map(|phi| phi.result).chain(block.ops.iter().flat_map(|op| op.defines.iter().copied()))
}

fn extended() -> Semantics {
    Semantics::new(vec![Format::Extended80], Format::Extended80, Precision::Dynamic, Rounding::Dynamic)
}

#[test]
fn test_peeling_clones_diamond_and_early_exit_phis() {
    let (body, values) = diamond();
    let loop_ = only_loop(&body);
    let changed = peeled(&body, &loop_, 2).unwrap().expect("peeled");
    assert_eq!(changed.blocks.len(), body.blocks.len() + 2 * loop_.body.len());
    let predecessors = loops::predecessors(&changed.blocks);
    for block in &changed.blocks {
        for phi in &block.phis {
            assert_eq!(phi.incoming.keys().copied().collect::<BTreeSet<_>>(), predecessors[&block.at]);
        }
    }
    let first = changed.block(changed.block(0).unwrap().succ[0]).unwrap();
    assert_eq!(first.phis[0].incoming, [(0, values[0])].into_iter().collect::<OrderedMap<_, _>>());
    let residual = changed.block(1).unwrap();
    assert_eq!(residual.phis[0].incoming.get(&5), Some(&values[5]));
    assert!(!residual.phis[0].incoming.contains_key(&0));
    assert_eq!(changed.block(6).unwrap().phis[0].incoming.len(), 6);
    let defined = changed.blocks.iter().flat_map(definitions).map(|value| value.id).collect::<Vec<_>>();
    assert_eq!(defined.len(), defined.iter().collect::<BTreeSet<_>>().len());
    assert_eq!(loops::loops(&changed.blocks, Some(changed.entry)), vec![loop_]);
    let owners = changed
        .blocks
        .iter()
        .flat_map(|block| definitions(block).map(move |value| (value, block.at)))
        .collect::<BTreeMap<_, _>>();
    let dominators = loops::dominators(&changed.blocks, Some(changed.entry));
    for block in &changed.blocks {
        for op in &block.ops {
            assert!(op.uses.iter().filter_map(|value| owners.get(value)).all(|owner| dominators[&block.at].contains(owner)));
        }
        for phi in &block.phis {
            assert!(phi
                .incoming
                .iter()
                .filter_map(|(source, value)| owners.get(value).map(|owner| (source, owner)))
                .all(|(source, owner)| dominators[source].contains(owner)));
        }
    }
}

#[test]
fn test_clones_read_their_own_values_and_do_not_duplicate_byte_ownership() {
    let (body, _) = diamond();
    let loop_ = only_loop(&body);
    let changed = peeled(&body, &loop_, 1).unwrap().expect("peeled");
    let originals = body.blocks.iter().map(|block| block.at).collect::<BTreeSet<_>>();
    let fresh = changed
        .blocks
        .iter()
        .filter(|block| !originals.contains(&block.at))
        .flat_map(definitions)
        .collect::<BTreeSet<_>>();
    for block in &changed.blocks {
        if originals.contains(&block.at) {
            continue;
        }
        for op in &block.ops {
            assert!(op.uses.iter().all(|value| fresh.contains(value)));
            assert!(op.inserted() && op.absorbed.is_empty());
            if let Some(Arg::Held(held)) = op.results.first() {
                assert_eq!(held.value, op.defines[0]);
            }
        }
    }
    assert_eq!(changed.block(3).unwrap().ops, body.block(3).unwrap().ops);
}

#[test]
fn test_peeling_refuses_floating_work_behind_an_internal_branch() {
    let (mut body, _) = diamond();
    let work = body.blocks.iter().position(|block| block.at == 3).unwrap();
    body.blocks[work].ops[0].floating = Some(extended());
    let loop_ = only_loop(&body);

    assert_eq!(peeled(&body, &loop_, 2).unwrap(), None);
}

#[test]
fn test_peeling_accepts_block_local_floating_values_behind_a_branch() {
    let (mut body, _) = diamond();
    let temporary = Value { id: 20, at: 30, flags: false, variable: 20, version: 1 };
    let mut load = Op::new(30, OpCode::Operation(Operation::FloatLoad), "fld", vec![temporary], vec![]);
    load.floating = Some(extended());
    load.kind = Kind::Fload;
    load.results = vec![Arg::Held(Held { value: temporary, width: 10 })];
    let mut store = Op::new(31, OpCode::Operation(Operation::FloatStore), "fstp", vec![], vec![temporary]);
    store.floating = Some(extended());
    store.kind = Kind::Fstore;
    store.args = vec![Arg::Held(Held { value: temporary, width: 10 })];
    let work = body.blocks.iter().position(|block| block.at == 3).unwrap();
    let rest = body.blocks[work].ops[1..].to_vec();
    body.blocks[work].ops = [vec![load, store], rest].concat();
    let loop_ = only_loop(&body);

    assert!(peeled(&body, &loop_, 1).unwrap().is_some());
}

#[test]
fn test_peeling_clones_pointer_identity_and_seed_facts() {
    let (mut body, values) = diamond();
    let (left, right, selected, stepped) = (values[2], values[3], values[4], values[5]);
    let object = MemoryObject {
        identity: Some(Identity::Tuple(vec![Identity::Int(7), Identity::Int(-16), Identity::Int(-4)])),
        extent: Some(12),
        ..MemoryObject::new(MemoryKind::Frame)
    };
    let provenance = Provenance::one_with_slice(object, 0, 1, 1, 1, BTreeSet::new()).unwrap();
    let interval = IntegerRange::new(0, 31, 2);
    body.pointer_values = BTreeSet::from([left, right, selected, stepped]);
    body.pointer_seeds = [(left, provenance.clone())].into_iter().collect();
    body.integer_ranges = [(left, interval.clone())].into_iter().collect();
    let loop_ = only_loop(&body);

    let changed = peeled(&body, &loop_, 1).unwrap().expect("peeled");

    let originals = body.blocks.iter().map(|block| block.at).collect::<BTreeSet<_>>();
    let copied_blocks = changed.blocks.iter().filter(|block| !originals.contains(&block.at)).collect::<Vec<_>>();
    let copied_results = copied_blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| [30, 40, 50].contains(&op.at) && !op.defines.is_empty())
        .map(|op| op.defines[0])
        .collect::<Vec<_>>();
    assert!(!copied_results.is_empty());
    assert!(copied_results.iter().all(|value| changed.pointer_values.contains(value)));
    let cloned_left = copied_blocks.iter().flat_map(|block| &block.ops).find(|op| op.at == 30).unwrap().defines[0];
    assert_eq!(changed.pointer_seeds.get(&cloned_left), Some(&provenance));
    assert_eq!(changed.integer_ranges.get(&cloned_left), Some(&interval));
}

#[test]
fn test_unclosed_loop_value_is_refused() {
    let (mut body, values) = diamond();
    let mut escape = body.block(3).unwrap().ops[0].clone();
    escape.uses = vec![values[1]];
    escape.args = vec![Arg::Held(Held { value: values[1], width: 2 })];
    body.blocks.last_mut().unwrap().ops = vec![escape];
    let loop_ = only_loop(&body);
    assert_eq!(peeled(&body, &loop_, 1).unwrap(), None);
}

#[test]
fn test_opaque_dispatch_is_not_cloned_as_an_ordinary_branch() {
    let (mut body, _) = diamond();
    let dispatch = body.blocks.iter().position(|block| block.at == 2).unwrap();
    body.blocks[dispatch].ops[0].kind = Kind::Call;
    let loop_ = only_loop(&body);
    assert_eq!(peeled(&body, &loop_, 1).unwrap(), None);
}

#[test]
fn test_nonpositive_peel_count_is_rejected() {
    for count in [0, -1] {
        let (body, _) = diamond();
        let loop_ = only_loop(&body);
        assert!(peeled(&body, &loop_, count).is_err());
    }
}
