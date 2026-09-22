//! Port of tests/test_unroll.py.
//!
//! Skipped: monkeypatching tests
//! (`test_unroll_rejects_growth_not_paid_for_by_dynamic_work`,
//! `test_peel_reports_the_gate_that_rejected_its_candidate`,
//! `test_peel_bounds_conditional_floating_clone_work`), and every test
//! reaching corpus, `mir.bodies`, cfront, transform, lower or lower_floats
//! (`test_c_matmul_unrolls_exact_multiblock_loops`,
//! `test_c_crc_specializes_constant_outer_bytes_and_keeps_inner_result`,
//! `test_c_nbody_peels_the_fixed_triangular_interaction_loop`,
//! `test_c_shellsort_retains_large_exact_loops_instead_of_cloning_every_store`,
//! `test_dead_inserted_store_needs_no_neighbor_to_take_its_bytes`,
//! `test_production_fpdeep_unrolls_through_lcssa_exits`,
//! `test_normal_pipeline_expands_and_folds_fpdeep_to_a_fixed_point`,
//! `test_fpcse_exact_ten_iteration_sum_folds_in_source_order`,
//! `test_runtime_input_loop_is_not_expanded_without_exact_folding`,
//! `test_fpdeep_unroll_preserves_order_and_fresh_definitions`,
//! `test_unrolled_latch_explicitly_skips_the_original_header`,
//! `test_fpdeep_expansion_exposes_exact_array_arithmetic`,
//! `test_fpdeep_exact_integer_arguments_keep_floating_checkpoints`,
//! `test_fpdeep_exact_stores_remove_their_arithmetic_chains`,
//! `test_integer_conversion_facts_require_exact_in_range_values`,
//! `test_expansion_provenance_does_not_allow_arbitrary_float_sequences`).

use super::*;
use std::rc::Rc;
use crate::backend::cpu;
use crate::model::ir::Operation;
use crate::model::mir::OpCode;

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
    let (original, result) = (Rc::new(original), Rc::new(result));
    let with = |name: &str| Where {
        costs: cpu::profile(name).expect("a known cpu").operations.clone(),
        ..Where::default()
    };

    assert!(_profitable(&original, &result, 1, 2, &with("386")));
    assert!(!_profitable(&original, &result, 1, 2, &with("P5")));
}
