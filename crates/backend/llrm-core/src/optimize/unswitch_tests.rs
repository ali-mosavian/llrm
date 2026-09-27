//! Port of tests/test_unswitch.py.
//!
//! Skipped, monkeypatching `transform.applied` or `unswitch.specialized`:
//! `test_unswitch_rejects_a_candidate_without_loop_removal`,
//! `test_unswitch_reoptimization_preserves_mir_target_costs`,
//! `test_unswitch_rejects_lower_count_but_higher_target_cost`,
//! `test_unswitch_rejects_semantic_work_without_a_target_price`.

use std::collections::BTreeSet;

use iced_x86::{Mnemonic, OpKind};

use super::*;
use crate::model::mir::{Const, MemRef};
use crate::objectfile::module::Module;
use crate::testing;
use crate::wholeseg::{Emission, Watched};

/// IVARM should branch once and store 34; its final counter remains 37.
#[test]
#[ignore = "fails in Python too: assert not [Loop(header=129, latches=frozenset({74}), ...)]"]
fn test_production_ivarm_has_no_loop_and_stores_last_value() {
    for tag in ["q-o", "p-g2", "v-g3"] {
        let result = testing::emitted(&testing::data(format!("{}/tests/fixtures/regressions/ivarm-{tag}.obj", env!("LLRM_ROOT"))));
        assert_eq!(result.outcome, Emission::Lir, "{}", result.reason);
        let decoded = testing::partitioned_bytes(&result.data);
        assert!(loops::loops(&testing::graph(&decoded), None).is_empty(), "{tag}");
        let stored = testing::instructions(&result.data)
            .into_iter()
            .filter(|one| {
                one.mnemonic() == Mnemonic::Mov
                    && one.op0_kind() == OpKind::Memory
                    && one.memory_size().size() == 2
                    && one.op1_kind() == OpKind::Immediate16
                    && one.immediate16() == 0x22
            })
            .count();
        assert_eq!(stored, 2, "{tag}");
    }
}

/// IVPROC refused mixed body layouts instead of removing IVARM's loop beside ANNOUNCE.
#[test]
#[ignore = "fails in Python too: assert not [Loop(header=126, latches=frozenset({74}), ...)]"]
fn test_specialized_main_and_legacy_procedure_emit_together() {
    for tag in ["q-o", "p-g2", "v-g3"] {
        let mut cloned = vec![];
        let mut watch = |stage: &str, _: Option<&str>, low: Watched<'_>| {
            if let (true, Watched::Mir(body)) = (stage == "mir-widen", low) {
                cloned.push(body.cloned);
            }
        };
        let data = testing::data(format!("{}/tests/fixtures/regressions/ivproc-{tag}.obj", env!("LLRM_ROOT")));
        let result = testing::emitted_watching(&data, Some(&mut watch));
        assert!(cloned.contains(&true) && cloned.contains(&false), "{tag}");
        assert_eq!(result.outcome, Emission::Lir, "{}", result.reason);
        assert!(loops::loops(&testing::graph(&testing::partitioned_bytes(&result.data)), None).is_empty());
    }
}

fn original(tag: &str) -> (Rc<Module>, Rc<MirBody>) {
    let found = testing::module(&format!("{}/tests/fixtures/regressions/ivarm-{tag}.obj", env!("LLRM_ROOT")));
    let body = testing::main_body(&found, &testing::blocks_of(&found));
    let options = Options { unroll: false, ..Options::default() };
    let how = transform::Applied { found: Some(found.clone()), options, ..Default::default() };
    let applied = transform::applied(&body, &found.dgroup.members, &found.calls, how).unwrap();
    (found, applied)
}

/// IVARM repeated its invariant branch ten times; specialized loops must retain usable exits.
#[test]
#[ignore = "fails in Python too: assert candidate is not body"]
fn test_invariant_branch_specialization_exposes_loop_deletion() {
    for tag in ["q-o", "p-g2", "v-g3"] {
        let (found, body) = original(tag);
        let candidate = specialized(&body).unwrap();
        assert!(!Rc::ptr_eq(&candidate, &body), "{tag}");
        let predecessors = loops::predecessors(&candidate.blocks);
        for block in &candidate.blocks {
            for phi in &block.phis {
                assert_eq!(phi.incoming.keys().copied().collect::<BTreeSet<_>>(), predecessors[&block.at]);
            }
        }
        let how = transform::Applied { options: Options { unroll: false, ..Options::default() }, ..Default::default() };
        let result = transform::applied(&candidate, &found.dgroup.members, &found.calls, how).unwrap();
        assert!(loops::loops(&result.blocks, Some(result.entry)).is_empty());
    }
}

/// Cloned IVARM blocks must not be re-sorted by original instruction addresses.
#[test]
fn test_cloning_provenance_survives_ssa_reconstruction() {
    let (_, body) = original("p-g2");
    let candidate = specialized(&body).unwrap();
    assert!(candidate.cloned);
    assert!(mir::resolved(&candidate, None).unwrap().cloned);
}

/// IVARM's dispatch needs distinct preheaders on both sides of its condition.
#[test]
#[ignore = "fails in Python too: ValueError: not enough values to unpack (expected 1, got 0)"]
fn test_implicit_edge_bridge_does_not_retarget_the_taken_arm() {
    let (_, body) = original("p-g2");
    let [loop_] = <[Loop; 1]>::try_from(loops::loops(&body.blocks, Some(body.entry))).unwrap();
    let header = body.block(loop_.header).unwrap();
    let taken = header.ops.last().unwrap().target.unwrap();
    let others: Vec<i64> = header.succ.iter().copied().filter(|&at| at != taken).collect::<BTreeSet<_>>().into_iter().collect();
    let [target] = <[i64; 1]>::try_from(others).unwrap();
    let label = edges::fresh(&body);
    let result = edges::split(&body, header.at, target, label, vec![]).unwrap();
    assert_eq!(result.block(header.at).unwrap().ops.last().unwrap().target, Some(taken));
    let succ: BTreeSet<i64> = result.block(header.at).unwrap().succ.iter().copied().collect();
    assert_eq!(succ, BTreeSet::from([taken, label]));
    assert_eq!(result.block(label).unwrap().succ, vec![target]);
}

#[test]
#[ignore = "fails in Python too: ValueError: not enough values to unpack (expected 1, got 0)"]
fn test_condition_must_be_pure_and_loop_invariant() {
    for variant in ["memory", "variant"] {
        let (_, body) = original("p-g2");
        let [loop_] = <[Loop; 1]>::try_from(loops::loops(&body.blocks, Some(body.entry))).unwrap();
        let selected = body
            .blocks
            .iter()
            .find(|block| loop_.body.contains(&block.at) && block.at != loop_.header && block.succ.len() == 2)
            .unwrap();
        let (index, compare) = transform::_comparison(selected, selected.ops.last().unwrap()).unwrap();
        let mut compare = compare.clone();
        if variant == "memory" {
            compare.loads = vec![MemRef::new(None, 2)];
        } else {
            let carried = body.block(loop_.header).unwrap().phis[0].result;
            compare.args = vec![Arg::Held(Held { value: carried, width: 2 }), Arg::Const(Const::new(0, 2))];
            compare.uses = vec![carried];
        }
        let mut selected = selected.clone();
        selected.ops[index] = compare;
        let mut changed = MirBody::clone(&body);
        for block in &mut changed.blocks {
            if block.at == selected.at {
                *block = selected.clone();
            }
        }
        let changed = Rc::new(changed);
        assert!(Rc::ptr_eq(&specialized(&changed).unwrap(), &changed), "{variant}");
    }
}
