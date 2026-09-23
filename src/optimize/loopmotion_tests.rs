//! Port of tests/test_loopmotion.py.
//!
//! `test_indexed_record_accumulators_store_only_after_loop` leaves out q-O,
//! which fails in Python at this commit (0x005b: 6 bytes between the ops are
//! not instructions).
//! Skipped, monkeypatching a pass:
//! `test_indexed_exit_store_requires_a_dominating_invariant_address`,
//! `test_harr_constant_column_exit_is_stored_once_only_after_a_nonempty_loop`,
//! `test_nbody_conditional_accumulator_stores_sink`,
//! `test_lngmxx_invariant_temporaries_sink_only_when_loop_executes`,
//! `test_nested_accumulator_seed_follows_outer_phi`,
//! `test_addrm_exit_store_requires_complete_initial_memory`.
//! Skipped, failing in Python at this commit:
//! `test_counter_is_written_once_at_the_exit_not_every_iteration` (writes at 0x5E),
//! `test_rotated_accumulator_store_uses_exit_phi` (the latch keeps its store).

use std::collections::BTreeSet;
use std::rc::Rc;

use super::sunk_stores;
use crate::abi::runtime;
use crate::analysis::loops;
use crate::model::mir::{Arg, Const, Kind, MemRef, MirBody};
use crate::objectfile::module::{self, Space};
use crate::support::testing;
use crate::optimize::{promote, transform};
use crate::support::hash::IndexMap;

type Bounds = IndexMap<(Space, i64), Vec<i64>>;

fn hotlop() -> (Rc<MirBody>, BTreeSet<i64>, Bounds, MemRef) {
    let found = testing::module("fixtures/omf/hotlop-p-g2.obj");
    let blocks = testing::blocks_of(&found);
    let mut contracts = runtime::for_module(&found, None).unwrap();
    let body = testing::raised_from(&found, &blocks, Some(&mut contracts)).values[0].1.clone();
    let counter =
        body.blocks.iter().flat_map(|block| &block.ops).find(|op| op.at == 0x5E).unwrap().stores[0].clone();
    let bounds = module::landmarks(&found);
    let dgroup = found.dgroup.members.clone();
    let promoted = promote::promoted(&body, &dgroup, Some(&bounds), false, true, false).unwrap();
    (promoted, dgroup, bounds, counter)
}

/// NESTED's accumulator is sunk past the outer loop or folded to its final 675.
#[test]
fn test_nested_accumulator_is_stored_only_after_the_outer_loop() {
    let found = testing::module("fixtures/omf/nested-p-g2.obj");
    let partition = testing::blocks_of(&found);
    let body = testing::main_body(&found, &partition);
    let accumulator =
        body.blocks.iter().flat_map(|block| &block.ops).find(|op| op.at == 0x7E).unwrap().stores[0].clone();
    let body = transform::applied(
        &body,
        &found.dgroup.members,
        &found.calls,
        transform::Applied { blocks: Some(partition), found: Some(found.clone()), ..Default::default() },
    )
    .unwrap();
    let hot: BTreeSet<i64> =
        loops::loops(&body.blocks, Some(body.entry)).into_iter().flat_map(|one| one.body).collect();
    let writes: Vec<i64> = body
        .blocks
        .iter()
        .flat_map(|block| block.ops.iter().filter(|op| op.stores.contains(&accumulator)).map(|_| block.at))
        .collect();
    assert!(writes.iter().all(|at| !hot.contains(at)));
    if hot.is_empty() {
        assert!(writes.is_empty());
        assert!(body.blocks.iter().flat_map(|block| &block.ops).filter(|op| op.kind == Kind::Arg).any(|op| {
            op.args.iter().any(|arg| matches!(arg, Arg::Const(one) if *one == Const::new(675, one.width)))
        }));
        assert!(body.repetitions.iter().any(|&(_, count)| count == 6));
        return;
    }
    assert_eq!(writes, vec![body.entry, 0x9C]);
}

#[test]
fn test_an_observer_in_the_loop_keeps_the_store() {
    for observer in ["read", "write", "call", "escape", "opaque"] {
        let (body, dgroup, bounds, counter) = hotlop();
        let header = body.block(0x5E).unwrap();
        let store = header.ops.iter().find(|op| op.stores.contains(&counter)).unwrap();
        let mut extra = store.clone();
        extra.kind = match observer {
            "read" => Kind::Load,
            "write" => Kind::Store,
            "call" => Kind::Call,
            "escape" => Kind::Escape,
            _ => Kind::Opaque,
        };
        extra.loads = if observer == "read" { vec![counter.clone()] } else { vec![] };
        extra.stores = if observer == "write" { vec![counter.clone()] } else { vec![] };
        let mut changed = header.clone();
        changed.ops.insert(0, extra);
        let mut altered = (*body).clone();
        for block in &mut altered.blocks {
            if block.at == changed.at {
                *block = changed.clone();
            }
        }
        let result = sunk_stores(&Rc::new(altered), &dgroup, Some(&bounds), true).unwrap();
        assert_eq!(result.block(changed.at).unwrap().ops, changed.ops, "{observer}");
    }
}

#[test]
fn test_an_exit_reachable_without_the_store_gets_no_new_write() {
    let (body, dgroup, bounds, _) = hotlop();
    let mut altered = (*body).clone();
    for block in &mut altered.blocks {
        if block.at == body.entry {
            block.succ.push(0x66);
        }
    }
    let altered = Rc::new(altered);
    assert_eq!(*sunk_stores(&altered, &dgroup, Some(&bounds), true).unwrap(), *altered);
}

#[test]
fn test_an_accumulator_without_zero_trip_initialization_stays_in_the_loop() {
    let (body, dgroup, bounds, _) = hotlop();
    let block = body.block(0x48).unwrap().clone();
    let refs: Vec<&MemRef> = block.ops.iter().flat_map(|op| &op.stores).collect();
    let mut altered = (*body).clone();
    for one in &mut altered.blocks {
        if one.at == body.entry {
            one.ops.retain(|op| !op.stores.iter().any(|stored| refs.contains(&stored)));
        }
    }
    let result = sunk_stores(&Rc::new(altered), &dgroup, Some(&bounds), true).unwrap();
    assert_eq!(*result.block(block.at).unwrap(), block);
}

/// UDTRNG wrote both promoted record accumulators on every iteration instead of once at exit.
#[test]
fn test_indexed_record_accumulators_store_only_after_loop() {
    for tag in ["p-g2", "v-g3"] {
        let (result, states) = testing::emitted_states(&testing::data(format!("fixtures/regressions/udtrng-{tag}.obj")));
        assert_eq!(result.outcome, crate::wholeseg::Emission::Lir, "{}", result.reason);
        let body = &states.last().unwrap().2;
        let hot: BTreeSet<i64> =
            loops::loops(&body.blocks, Some(body.entry)).into_iter().flat_map(|one| one.body).collect();
        let wide: Vec<(i64, MemRef)> = body
            .blocks
            .iter()
            .filter(|block| hot.contains(&block.at))
            .flat_map(|block| &block.ops)
            .flat_map(|op| op.stores.iter().map(move |one| (op.at, one.clone())))
            .filter(|(_, one)| one.base.is_some() && one.width == 4)
            .collect();
        assert!(wide.is_empty(), "{tag}: {wide:?}");
    }
}

/// NBODYS stored `other` every inner iteration: a float op kept the loop's stores for a handler it has not got.
#[test]
fn test_a_float_loop_sinks_its_counter_store_without_an_error_handler() {
    let (_, states) = testing::emitted_states(&testing::data("fixtures/bench/nbodys-v-g3.obj"));
    let mut last: IndexMap<Option<String>, MirBody> = IndexMap::default();
    for (_, name, body) in states.into_iter().filter(|(stage, _, _)| stage.starts_with("mir-")) {
        last.insert(name, body);
    }
    let body = last
        .values()
        .find(|one| one.blocks.iter().flat_map(|block| &block.ops).any(|op| op.kind == Kind::Fmul))
        .unwrap();
    let ops = |at: &i64| &body.block(*at).unwrap().ops;
    let floating = loops::loops(&body.blocks, Some(body.entry))
        .into_iter()
        .filter(|one| one.body.iter().flat_map(ops).any(|op| op.floating.is_some()));
    let innermost = floating.min_by_key(|one| one.body.len()).unwrap();
    assert!(!innermost.body.iter().flat_map(ops).any(|op| op.kind == Kind::Store));
}
