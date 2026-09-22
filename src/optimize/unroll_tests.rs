//! Port of tests/test_unroll.py.
//!
//! Skipped, monkeypatching a pass:
//! `test_unroll_rejects_growth_not_paid_for_by_dynamic_work`,
//! `test_peel_reports_the_gate_that_rejected_its_candidate`,
//! `test_peel_bounds_conditional_floating_clone_work`,
//! `test_fpdeep_exact_integer_arguments_keep_floating_checkpoints`,
//! `test_integer_conversion_facts_require_exact_in_range_values`.
//! Skipped, needing `tools/quality.py`:
//! `test_c_matmul_unrolls_exact_multiblock_loops`,
//! `test_c_crc_specializes_constant_outer_bytes_and_keeps_inner_result`,
//! `test_c_nbody_peels_the_fixed_triangular_interaction_loop`,
//! `test_c_shellsort_retains_large_exact_loops_instead_of_cloning_every_store`.
//! Skipped, failing in Python at this commit:
//! `test_production_fpdeep_unrolls_through_lcssa_exits`,
//! `test_runtime_input_loop_is_not_expanded_without_exact_folding`,
//! `test_fpdeep_unroll_preserves_order_and_fresh_definitions`,
//! `test_unrolled_latch_explicitly_skips_the_original_header`,
//! `test_fpdeep_expansion_exposes_exact_array_arithmetic`,
//! `test_fpdeep_exact_stores_remove_their_arithmetic_chains`,
//! `test_expansion_provenance_does_not_allow_arbitrary_float_sequences`.

use std::collections::BTreeSet;

use super::*;
use crate::analysis::loops;
use crate::backend::{cpu, lower_floats};
use crate::model::ir::Operation;
use crate::model::mir::{Arg, Cell, Const, MemRef, MirBlock, OpCode};
use crate::model::passes::Options;
use crate::objectfile::module::{Addr, Module, Space};
use crate::optimize::{testcorpus, transform};

fn op(at: i64, operation: Operation, name: &str, kind: Kind) -> Op {
    let mut op = Op::new(at, OpCode::Operation(operation), name, vec![], vec![]);
    op.kind = kind;
    op
}

#[test]
fn test_unroll_profitability_uses_the_selected_cpu() {
    let add = op(1, Operation::Binary, "add", Kind::Add);
    let mut branch = op(1, Operation::Branch, "jne", Kind::Branch);
    branch.target = Some(1);
    let original = MirBody::new(
        0,
        vec![
            MirBlock::new(0, vec![], vec![], vec![1]),
            MirBlock::new(1, vec![], vec![add, branch], vec![1, 2]),
            MirBlock::new(2, vec![], vec![], vec![]),
        ],
    );
    let moves = (0..3).map(|at| op(at, Operation::Move, "mov", Kind::Copy)).collect();
    let mut result = MirBody::new(
        0,
        vec![MirBlock::new(0, vec![], moves, vec![2]), MirBlock::new(2, vec![], vec![], vec![])],
    );
    result.repetitions = vec![(1, 2)];
    let with = |name: &str| Where {
        costs: cpu::profile(name).expect("a known cpu").operations.clone(),
        ..Where::default()
    };

    assert!(_profitable(&original, &result, 1, 2, &with("386")));
    assert!(!_profitable(&original, &result, 1, 2, &with("P5")));
}

/// FPCSE retained dead unrolled stores because a zero-byte clone could not donate bytes to its neighbor.
#[test]
fn test_dead_inserted_store_needs_no_neighbor_to_take_its_bytes() {
    let cell = MemRef::new(Some(Addr { index: 5, ..Addr::new(Space::Segment, 0) }), 4);
    let store = |at: i64, value: i64| {
        let mut one = op(at, Operation::Move, "", Kind::Store);
        one.args = vec![Arg::Const(Const::new(value, 4))];
        one.results = vec![Arg::Cell(Cell { r#ref: cell.clone() })];
        one.stores = vec![cell.clone()];
        one
    };
    for checkpoint in [false, true] {
        let (first, last) = (store(10, 1), store(20, 2));
        let mut ops = vec![first];
        if checkpoint {
            ops.push(op(15, Operation::Nothing, "", Kind::Fcheck));
        }
        ops.push(last.clone());
        let body = Rc::new(MirBody::new(0, vec![MirBlock::new(0, vec![], ops.clone(), vec![])]));
        let changed =
            transform::without_dead_stores(&body, &BTreeSet::from([5]), &IndexMap::default(), None, None, true)
                .unwrap();
        assert_eq!(changed.blocks[0].ops, if checkpoint { ops } else { vec![last] }, "{checkpoint}");
    }
}

fn applied(found: &Rc<Module>, body: &Rc<MirBody>, options: Options) -> Rc<MirBody> {
    transform::applied(
        body,
        &found.dgroup.members,
        &found.calls,
        transform::Applied { found: Some(found.clone()), options, ..Default::default() },
    )
    .unwrap()
}

/// FPDEEP's improvements previously required an out-of-band unroll wrapper.
#[test]
fn test_normal_pipeline_expands_and_folds_fpdeep_to_a_fixed_point() {
    let found = testcorpus::loaded("fixtures/omf/fpdeep-p-g2.obj");
    let original = testcorpus::main_body(&found, &testcorpus::partitioned(&found));
    let changed = applied(&found, &original, Options::default());
    assert_eq!(changed.repetitions, vec![(0x66, 3)]);
    assert!(loops::loops(&changed.blocks, Some(changed.entry)).is_empty());
    assert_eq!(applied(&found, &changed, Options::default()), changed);
    assert!(applied(&found, &original, Options { unroll: false, ..Default::default() }).repetitions.is_empty());
}

/// FPCSE computed its exact 487.5 sum ten times despite fitting the bounded expansion budget.
#[test]
fn test_fpcse_exact_ten_iteration_sum_folds_in_source_order() {
    let found = testcorpus::loaded("fixtures/omf/fpcse-p-g2.obj");
    let original = testcorpus::main_body(&found, &testcorpus::partitioned(&found));
    let changed = applied(&found, &original, Options::default());
    assert!(changed.repetitions.is_empty());
    assert!(loops::loops(&changed.blocks, Some(changed.entry)).is_empty());
    let ops: Vec<&Op> = changed.blocks.iter().flat_map(|block| &block.ops).collect();
    assert!(!ops.iter().any(|one| one.floating.is_some()));
    let bits = Arg::Const(Const::new(487.5_f32.to_bits(), 4));
    assert!(ops.iter().any(|one| one.kind == Kind::Store && one.at == 0xA1 && one.args == [bits.clone()]));
    lower_floats::checked(&changed).unwrap();
}
