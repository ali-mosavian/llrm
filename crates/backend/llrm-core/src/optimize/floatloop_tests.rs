//! Port of tests/test_float_loop_exit.py.

use std::collections::BTreeSet;

use num_bigint::BigInt;

use super::*;
use crate::model::ir::Operation;
use crate::model::mir::{Arg, Const, Kind, MemRef, MirBody, Op, OpCode, Value};
use crate::model::passes::Options;
use crate::objectfile::module::Module;
use crate::optimize::{loopmotion, transform};
use crate::testing;

fn fpcse(tag: &str) -> (Rc<Module>, Rc<MirBody>) {
    let found = testing::module(&format!("{}/tests/fixtures/omf/fpcse-{tag}.obj", env!("LLRM_ROOT")));
    let body = testing::main_body(&found, &testing::blocks_of(&found));
    (found, body)
}

fn all(body: &MirBody) -> Vec<Op> {
    testing::ops(body)
}

fn constant(n: i64, width: u32) -> Vec<Arg> {
    vec![Arg::Const(Const::new(n, width))]
}

#[test]
fn test_checkpoint_with_additional_effects_is_not_ignored() {
    for effect in ["value", "memory", "barrier"] {
        let mut op = Op::new(0, OpCode::Operation(Operation::Nothing), "", vec![], vec![]);
        op.kind = Kind::Fcheck;
        match effect {
            "value" => op.defines = vec![Value::new(1, 0)],
            "memory" => op.stores = vec![MemRef::new(None, 2)],
            _ => op.op = Some(OpCode::Operation(Operation::Barrier)),
        }
        assert!(!floatfacts::checkpoint(&op), "{effect}");
        let repeated = floatfacts::repeated(&[op], &BigInt::from(1), &Default::default(), &BTreeSet::new(), None, None);
        assert!(repeated.is_none(), "{effect}");
    }
}

/// FPCSE seeds 438.75 and retains the strict final iteration to reach 487.5.
#[test]
fn test_emitted_final_answer_has_the_correct_symbol() {
    for tag in ["p-g2", "q-o", "v-g3"] {
        let (_, body) = fpcse(tag);
        let accumulator = body
            .blocks
            .iter()
            .flat_map(|block| block.ops.iter().rev())
            .find(|op| op.kind == Kind::Fstore)
            .unwrap()
            .stores[0]
            .clone();
        let counter = body
            .blocks
            .iter()
            .filter(|block| !block.phis.is_empty())
            .flat_map(|block| &block.ops)
            .find(|op| !op.stores.is_empty())
            .unwrap()
            .stores[0]
            .clone();
        let (result, states) = testing::emitted_states(&testing::data(format!("{}/tests/fixtures/omf/fpcse-{tag}.obj", env!("LLRM_ROOT"))));
        assert_eq!(result.outcome, crate::wholeseg::Emission::Lir, "{}", result.reason);
        let states: Vec<MirBody> =
            states.into_iter().filter(|(stage, _, _)| stage == "mir-r01-floatloop").map(|(_, _, one)| one).collect();
        let [specialized] = <[MirBody; 1]>::try_from(states).unwrap();
        assert!(loops::loops(&specialized.blocks, Some(specialized.entry)).is_empty());
        let ops = all(&specialized);
        let seeds: Vec<&Op> =
            ops.iter().filter(|op| op.kind == Kind::Store && op.args == constant(0x43DB6000, 4)).collect();
        let [seed] = <[&Op; 1]>::try_from(seeds).unwrap();
        assert!(seed.stores[0].addr == accumulator.addr && seed.symbol == Some(true), "{tag}");
        assert!(ops.iter().any(|op| op.kind == Kind::Fstore && op.stores.contains(&accumulator)));
        let finals: Vec<&Op> = ops.iter().filter(|op| op.kind == Kind::Store && op.args == constant(11, 2)).collect();
        let [final_counter] = <[&Op; 1]>::try_from(finals).unwrap();
        assert_eq!(final_counter.stores[0].addr, counter.addr);
    }
}

/// FPCSE computes 487.5, but previously repeated its exact FP body ten times.
#[test]
fn test_exact_loop_retains_checkpoint_and_final_iteration() {
    for tag in ["p-g2", "q-o", "v-g3"] {
        let (found, body) = fpcse(tag);
        let (dgroup, calls) = (&found.dgroup.members, &found.calls);
        let first = loops::loops(&body.blocks, Some(body.entry)).remove(0);
        let latch = body.block(*first.latches.iter().next().unwrap()).unwrap();
        let changed = specialized(&body, dgroup, calls).unwrap();
        assert!(!Rc::ptr_eq(&changed, &body));
        assert!(loops::loops(&changed.blocks, Some(changed.entry)).is_empty());
        let emitted = changed.block(latch.at).unwrap();
        let floating = |ops: &[Op]| ops.iter().filter(|op| op.floating.is_some()).cloned().collect::<Vec<_>>();
        let original = floating(&latch.ops);
        assert_eq!(floating(&emitted.ops), original);
        let observed = |ops: &[Op]| {
            ops.iter().filter(|op| op.floating.is_some() || op.kind == Kind::Fcheck).cloned().collect::<Vec<_>>()
        };
        assert_eq!(observed(&emitted.ops), observed(&latch.ops));
        assert_eq!(emitted.ops[0], original[0]);
        let accumulator = &latch.ops.iter().rev().find(|op| op.kind == Kind::Fstore).unwrap().stores[0];
        let seed = emitted.ops.iter().find(|op| op.kind == Kind::Store && op.stores.contains(accumulator)).unwrap();
        assert_eq!(seed.args, constant(0x43DB6000, 4)); // 438.75 before the final iteration
        let facts = floatfacts::known(&changed, dgroup, calls, None);
        let stored = emitted.ops.iter().rev().find(|op| op.kind == Kind::Fstore).unwrap();
        let Arg::Held(value) = &stored.args[0] else { panic!("{:?}", stored.args[0]) };
        let format = stored.floating.as_ref().unwrap().result;
        assert_eq!(floatfacts::encoded(&facts[&value.value], format), Some(BigInt::from(0x43F3C000)), "{tag}");
    }
}

/// FPCSE's generic-unroll test needs the loop before FloatLoop consumes it.
#[test]
fn test_floatloop_can_be_disabled_for_stage_bisection() {
    let (found, body) = fpcse("p-g2");
    let applied = |options: Options| {
        let only = Some("floatloop".to_owned());
        let how = transform::Applied { found: Some(found.clone()), options, only, ..Default::default() };
        transform::applied(&body, &found.dgroup.members, &found.calls, how).unwrap()
    };
    let enabled = applied(crate::model::passes::O2());
    let disabled = applied(Options { floatloop: false, ..Options::default() });
    assert!(loops::loops(&enabled.blocks, Some(enabled.entry)).is_empty());
    assert!(!loops::loops(&disabled.blocks, Some(disabled.entry)).is_empty());
}

/// A pending FP exception reaching ON ERROR must still see FPCSE's s=0 and first counter value.
#[test]
fn test_checkpoint_keeps_initial_memory_and_counter_stores_for_an_error_handler() {
    let (found, body) = fpcse("p-g2");
    let (dgroup, calls) = (&found.dgroup.members, &found.calls);
    let header = body.blocks.iter().find(|block| !block.phis.is_empty()).unwrap();
    let counter_store = header.ops.iter().find(|op| !op.stores.is_empty()).unwrap();
    for handles_errors in [true, false] {
        let sunk = loopmotion::sunk_stores(&body, dgroup, None, handles_errors).unwrap();
        assert_eq!(sunk.block(header.at).unwrap().ops.contains(counter_store), handles_errors);
        let changed = specialized(&body, dgroup, calls).unwrap();
        let changed = transform::without_dead_stores(&changed, dgroup, calls, None, None, handles_errors).unwrap();
        let entry = changed.block(changed.entry).unwrap();
        let kept = entry.ops.iter().any(|op| op.kind == Kind::Store && op.args == constant(0, 4));
        assert!(kept || !handles_errors);
    }
}

/// Do not turn an inexact or externally observed recurrence into a guessed final iteration.
#[test]
fn test_unproved_or_observable_iterations_remain() {
    let (found, body) = fpcse("p-g2");
    let header = body.blocks.iter().find(|block| !block.phis.is_empty()).unwrap();
    let condition = header.ops.iter().find(|op| op.kind == Kind::Sub).unwrap().defines[0];
    for change in ["inexact", "zero_trip", "one_trip", "call", "flags"] {
        let altered = |op: &Op| {
            let mut op = op.clone();
            if change == "inexact" && op.kind == Kind::Store && op.args == constant(0x41000000, 4) {
                op.args = constant(0x40E00000, 4); // 6/7 is not exact
            } else if matches!(change, "zero_trip" | "one_trip")
                && op.kind == Kind::Sub
                && op.args.last() == Some(&Arg::Const(Const::new(10, 2)))
            {
                op.args = vec![op.args[0].clone(), Arg::Const(Const::new(i64::from(change == "one_trip"), 2))];
            } else if change == "call" && op.kind == Kind::Fadd {
                op.kind = Kind::Call;
                op.floating = None;
            } else if change == "flags" && op.kind == Kind::Arg {
                op.uses.push(condition);
            }
            op
        };
        let mut changed = MirBody::clone(&body);
        for block in &mut changed.blocks {
            block.ops = block.ops.iter().map(altered).collect();
        }
        let changed = Rc::new(changed);
        let result = specialized(&changed, &found.dgroup.members, &found.calls).unwrap();
        assert!(Rc::ptr_eq(&result, &changed), "{change}");
    }
}
