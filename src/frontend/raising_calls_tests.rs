//! Port of `tests/test_raising_calls.py`.
//!
//! Skipped, needing `transform.applied` or `wholeseg`:
//! `test_nbody_timer_does_not_keep_arithmetic_scratch_values_live`,
//! `test_long_division_setup_is_not_counted_as_stack_arguments`,
//! `test_optimization_does_not_move_nbody_store_before_its_definition`,
//! `test_recovered_memory_arguments_keep_their_relocations`,
//! `test_nbody_computed_loop_limit_is_a_native_comparison`.
//! Skipped, needing `asm._divide_fields`:
//! `test_divide_relocation_survives_index_value_replacement`.
//! Skipped, monkeypatching `mir._flags_after`:
//! `test_captured_comparison_retains_runtime_synthesized_flags`.

use std::collections::BTreeSet;

use super::*;
use crate::abi::runtime::{self, Contract, Reg};
use crate::legacy::calls;
use crate::model::mir::{Arg, MirBody};
use crate::objectfile::module::{Addr, Space};
use crate::support::hash::IndexMap;
use crate::support::testing::{self, nth, ops, width};

const NBODY_STACK: &str = "fixtures/regressions/nbody-stack-p-g2.obj";

fn nbody() -> MirBody {
    nth(&testing::raised(NBODY_STACK), 0)
}

fn wide_held(arg: &Arg) -> bool {
    matches!(arg, Arg::Held(one) if one.width == 4)
}

fn held(arg: &Arg) -> mir::Value {
    let Arg::Held(one) = arg else { panic!("{arg:?}") };
    one.value
}

/// Nbody's classified memory multiplies remained frozen machine sites, blocking forwarding.
#[test]
fn test_nbody_all_runtime_multiplies_are_scalar_values() {
    let found = testing::loaded(NBODY_STACK).unwrap();
    let body = nbody();
    for (&at, name) in &found.calls {
        if name != calls::MULTIPLY {
            continue;
        }
        let op = ops(&body).into_iter().find(|op| op.at == at && op.kind == Kind::Mul).unwrap();
        assert!(!op.source_backed && op.loads.is_empty());
        assert!(op.results.len() == 1 && width(&op.results[0]) == 4);
        assert!(op.args.iter().all(wide_held));
    }
}

/// Nbody's final force division stayed opaque solely because dead phis carried call clobbers.
#[test]
fn test_unused_loop_clobbers_do_not_hide_nbody_division() {
    let ops = ops(&nbody());
    assert!(ops.iter().any(|op| op.at == 0x204 && op.kind == Kind::Divmod));
    assert!(!ops.iter().any(|op| op.at == 0x204 && op.kind == Kind::Call));
}

/// Forwarded nbody velocity refused at 0x25f when frozen division required memory.
#[test]
fn test_nbody_classified_divide_consumes_captured_values() {
    let ops = ops(&nbody());
    let divide = ops.iter().find(|op| op.at == 0x26b && op.kind == Kind::Divmod).unwrap();
    assert!(!divide.source_backed && divide.loads.is_empty());
    assert!(divide.args.iter().all(wide_held));
    let captured = ops.iter().find(|op| op.at == 0x267 && op.kind == Kind::Load).unwrap();
    assert_eq!(captured.results[0], divide.args[0]);
}

#[test]
fn test_computed_divisions_are_values_not_runtime_calls() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        let raised = testing::raised(format!("fixtures/omf/chain-{tag}.obj").to_lowercase());
        let divisions: Vec<Op> =
            testing::all_ops(&raised).into_iter().filter(|op| op.kind == Kind::Divmod && !op.source_backed).collect();
        assert!(!divisions.is_empty(), "{tag}");
        for op in &divisions {
            assert!(op.args.len() == 2 && op.results.len() == 2, "{tag}");
            assert!(op.args.iter().chain(&op.results).all(wide_held), "{tag}");
            assert!(op.loads.is_empty() && op.stores.is_empty(), "{tag}");
        }
        for (_, body) in &raised.values {
            let defined: BTreeSet<_> = ops(body).iter().flat_map(|op| op.defines.clone()).collect();
            for op in ops(body).iter().filter(|op| divisions.contains(op)) {
                assert!(op.uses.iter().all(|value| defined.contains(value)), "{tag}");
            }
        }
    }
}

#[test]
fn test_nested_multiply_consumes_values_without_stealing_outer_arguments() {
    let ops = ops(&nbody());
    let product = ops.iter().find(|op| op.at == 0x1cd && op.kind == Kind::Mul).unwrap();
    let division = ops.iter().find(|op| op.at == 0x1d4 && op.kind == Kind::Divmod).unwrap();
    assert!(!product.source_backed && product.results.len() == 1);
    assert_eq!(width(&product.results[0]), 4);
    assert!(product.args.len() == 2 && product.args.iter().all(wide_held));
    let definitions: IndexMap<mir::Value, &Op> =
        ops.iter().flat_map(|op| op.defines.iter().map(move |value| (*value, op))).collect();
    let divisor = definitions[&held(&division.args[1])];
    assert_eq!(divisor.kind, Kind::Concat);
    assert_eq!(divisor.args.iter().map(|arg| definitions[&held(arg)].at).collect::<Vec<_>>(), [0x1be, 0x1c0]);
    assert!(divisor.args.iter().all(|arg| definitions[&held(arg)].kind == Kind::Copy));
}

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
