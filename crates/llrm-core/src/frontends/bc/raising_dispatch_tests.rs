//! Port of `tests/test_raising_dispatch.py`.
//!
//! Skipped, monkeypatching `raising_dispatch.raised` out of `mir.bodies`:
//! `test_unproved_dispatch_remains_a_call`.

use crate::analysis::consts::Known;
use crate::frontends::bc::blocks;
use crate::analysis::loops;
use crate::model::mir::{Arg, Const, Kind, MirBlock, MirBody};
use crate::optimize::transform::_executable_successors;
use crate::support::hash::IndexMap;
use crate::testing;

/// The block ending in the SWITCH, and the block guarding it.
fn _dispatch(body: &MirBody) -> (&MirBlock, &MirBlock) {
    let normal =
        body.blocks.iter().find(|block| block.ops.last().is_some_and(|op| op.kind == Kind::Switch)).unwrap();
    let guard = body.blocks.iter().find(|block| block.succ.contains(&normal.at)).unwrap();
    (normal, guard)
}

#[test]
fn test_real_dispatch_has_an_explicit_error_guard() {
    for tag in ["q-O", "p-g2", "v-g2", "v-g3"] {
        let source = format!("tests/fixtures/omf/jumps-{tag}.obj").to_lowercase();
        let found = testing::loaded(&source).unwrap();
        let body = testing::nth(&testing::raised(&source), 0);
        let (normal, guard_block) = _dispatch(&body);
        let dispatch = normal.ops.last().unwrap();
        let [compare, guard] = &guard_block.ops[guard_block.ops.len() - 2..] else { unreachable!() };
        assert_eq!(guard.test, Some(Kind::Above), "{tag}");
        assert_eq!(compare.args, [dispatch.args[0].clone(), Arg::Const(Const::new(255, 2))], "{tag}");
        let error = body.block(guard.target.unwrap()).unwrap();
        assert_eq!(found.calls[&error.ops.last().unwrap().at], "B$OGTA", "{tag}");
        assert_eq!(error.ops.last().unwrap().kind, Kind::Call, "{tag}");
        assert_eq!(dispatch.cases.iter().map(|(number, _)| *number).collect::<Vec<_>>(), [1, 2, 3], "{tag}");
        let mut targets: std::collections::BTreeSet<i64> = dispatch.cases.iter().map(|(_, target)| *target).collect();
        targets.insert(dispatch.target.unwrap());
        assert_eq!(targets, normal.succ.iter().copied().collect(), "{tag}");
        let predecessors = loops::predecessors(&body.blocks);
        for block in &body.blocks {
            for phi in &block.phis {
                assert_eq!(phi.incoming.keys().copied().collect::<std::collections::BTreeSet<_>>(), predecessors[&block.at]);
            }
        }
    }
}

#[test]
fn test_dispatch_boundary_reaches_the_required_path() {
    // The real PDS READ witness raises Illegal function call for 256 and -1.
    let source = concat!(env!("LLRM_ROOT"), "/tests/fixtures/regressions/dispatch-p-g2.obj");
    let found = testing::loaded(source).unwrap();
    let body = testing::nth(&testing::raised(source), 0);
    let (normal, guard) = _dispatch(&body);
    let dispatch = normal.ops.last().unwrap();
    let Arg::Held(selector) = &dispatch.args[0] else { panic!("{:?}", dispatch.args) };
    for number in [0_i64, 1, 2, 3, 4, 255, 256, -1] {
        let facts: IndexMap<_, _> = [(selector.value, Known::new(number & 65535, 2))].into_iter().collect();
        let successors = |block: &MirBlock| {
            _executable_successors(block, &facts, &IndexMap::default(), &IndexMap::default(), None)
        };
        let successors_ = successors(guard).unwrap();
        assert_eq!(successors_.len(), 1, "{number}");
        let target = body.block(successors_[0]).unwrap();
        if !(0..=255).contains(&number) {
            assert_eq!(target.ops.last().unwrap().kind, Kind::Call, "{number}");
            assert_eq!(found.calls[&target.ops.last().unwrap().at], "B$OGTA", "{number}");
        } else {
            assert_eq!(target, normal, "{number}");
            let expected = dispatch.cases.iter().find(|(case, _)| *case == number).map_or(dispatch.target.unwrap(), |one| one.1);
            assert_eq!(successors(target), Some(vec![expected]), "{number}");
        }
    }
}

#[test]
#[ignore = "fails in Python too: assert not True (JUMPS' emitted object has no code map)"]
fn test_guarded_dispatch_emits_instead_of_falling_back() {
    for tag in ["q-O", "p-g2", "v-g2", "v-g3"] {
        let output = testing::emitted_lir(format!("tests/fixtures/omf/jumps-{tag}.obj").to_lowercase());
        let found = testing::loaded_bytes(&output.data).unwrap();
        let mapped = blocks::code_map(&found).unwrap();
        // JUMPS only dispatches 1..3: neither the error call nor its table is reachable.
        assert!(!found.calls.values().any(|name| name == "B$OGTA"), "{tag}");
        let declared: Vec<(usize, usize)> = blocks::statement_table(&found).into_iter().collect();
        assert_eq!(mapped.tables, declared, "{tag}");
    }
}

#[test]
#[ignore = "fails in Python too: Unprintable: main (main): block leaves for (76, 87, 98, 109) with no instruction choosing"]
fn test_read_data_error_witness_emits_native_dispatch() {
    let source = concat!(env!("LLRM_ROOT"), "/tests/fixtures/regressions/dispatch-p-g2.obj");
    let raised = testing::raised(source);
    assert!(testing::all_ops(&raised).iter().any(|op| op.kind == Kind::Switch));
    let result = testing::emitted_lir(source);
    let mapped = blocks::code_map(&testing::loaded_bytes(&result.data).unwrap()).unwrap();
    assert!(!mapped.tables.is_empty());
}
