//! Port of `tests/test_raising_calls.py`.
//!
//! Skipped, needing `mir.bodies`, `transform.applied` or `wholeseg`, which
//! are not ported:
//! `test_nbody_timer_does_not_keep_arithmetic_scratch_values_live`,
//! `test_long_division_setup_is_not_counted_as_stack_arguments`,
//! `test_nbody_all_runtime_multiplies_are_scalar_values`,
//! `test_unused_loop_clobbers_do_not_hide_nbody_division`,
//! `test_optimization_does_not_move_nbody_store_before_its_definition`,
//! `test_divide_relocation_survives_index_value_replacement`,
//! `test_nbody_classified_divide_consumes_captured_values`,
//! `test_computed_divisions_are_values_not_runtime_calls`,
//! `test_recovered_memory_arguments_keep_their_relocations`,
//! `test_nested_multiply_consumes_values_without_stealing_outer_arguments`,
//! `test_nbody_computed_loop_limit_is_a_native_comparison`,
//! `test_captured_comparison_retains_runtime_synthesized_flags`.

use std::collections::BTreeSet;

use super::*;
use crate::abi::runtime::{self, Contract, Reg};
use crate::objectfile::module::{Addr, Space};

#[test]
fn test_known_call_inputs_do_not_establish_unknown_call_effects() {
    // Fixing NBODY's phantom timer inputs must not invent preserved registers.
    for inputs in [None, Some(BTreeSet::new()), Some(BTreeSet::from([Reg::Cx]))] {
        let contract = Contract { inputs: inputs.clone(), ..runtime::worst("unresolved") };
        let touched = mir::call_touches(Some("unresolved"), Some(&contract));
        let Some(inputs) = inputs else {
            assert!(touched.is_none());
            continue;
        };
        let (defines, uses) = touched.unwrap();
        let mut every: BTreeSet<Register> = mir::TRACKED.into_iter().collect();
        every.insert(mir::FLAGS);
        assert_eq!(defines, every);
        assert_eq!(uses, inputs.into_iter().map(|one| mir::from_contract(one).unwrap()).collect());
    }
}

#[test]
fn test_memory_argument_capture_requires_adjacent_pushes() {
    // A separated pair must keep its original snapshots, not reread both
    // words at the second push.
    let low = MemRef::new(Some(Addr { index: 5, ..Addr::new(Space::Segment, 4) }), 2);
    let high = MemRef { addr: low.addr.map(|addr| addr.plus(2)), ..low.clone() };
    let push = |at: i64, r#ref: &MemRef| {
        let mut made = Op::new(at, OpCode::Operation(Operation::Push), "push", Vec::new(), Vec::new());
        made.kind = Kind::Arg;
        made.args = vec![Arg::Cell(Cell { r#ref: r#ref.clone() })];
        made.loads = vec![r#ref.clone()];
        mir::raising_occurrence(&made, (at, at + 3), Vec::new(), None)
    };
    for separated in [false, true] {
        let (first, second) = (push(0, &high), push(if separated { 6 } else { 3 }, &low));
        let answer = _whole_memory(&[&first, &second]);
        assert_eq!(answer, if separated { None } else { Some(MemRef { width: 4, ..low.clone() }) });
    }
}
