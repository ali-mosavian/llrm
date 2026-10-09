//! `counting` had no tests of its own; these place a loop's exit value and
//! skip test in its preheader, as a rotation does, and run @f before and
//! after on no trip, one, and many.

use std::collections::BTreeSet;

use llrm_analysis::cfg;
use llrm_analysis::graph::loops;
use llrm_analysis::induction::{self, control_replacement};
use llrm_analysis::memory::Unit;
use llrm_analysis::testing::{DOS, layout};
use llrm_mir::module::{Module, Operand};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::Outer;

use super::{Seeds, leaving, skip_guard};
use crate::testing::{parsed, printed, results};

/// `i` from `start` while `i < bound`, its exit value returned through the
/// exit's phi.
fn counting(
    start: &str,
    bound: &str,
) -> String {
    format!(
        "{DOS}define i16 @f(i16 %s, i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ {start}, %b0 ], [ %next, %b2 ]
  %c = icmp slt i16 %i, {bound}
  br i1 %c, label %b2, label %b3

b2:
  %next = add nsw i16 %i, 1
  br label %b1

b3:
  %e = phi i16 [ %i, %b1 ]
  ret i16 %e
}}
"
    )
}

/// @f's exit phi reading `leaving`'s values, and b0 branching past the loop
/// where `skip_guard` says it runs no trip.
fn guarded(module: &mut Module) {
    let (layout, outer) = (layout(module), Outer::of(module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let found = loops::loops(&cfg::graph(function), None);
    let [loop_] = &found[..] else { panic!("one loop") };
    let proofs = induction::counted(
        &llrm_analysis::testing::with_registers(Unit::within(context, &layout, function, &outer)),
        loop_,
        None,
        false,
    );
    let [proof] = &proofs[..] else { panic!("one proof") };
    let replacement = control_replacement(
        &llrm_analysis::testing::with_registers(Unit::within(context, &layout, function, &outer)),
        loop_,
        proof,
        &BTreeSet::new(),
    )
    .expect("replaceable");
    let (preheader, header, exit) =
        (cfg::block(proof.preheader.expect("a preheader")), cfg::block(loop_.header), cfg::block(proof.exit));
    let jump = function.terminator(preheader).expect("a branch");
    let mut seeds = Seeds { context, function, at: jump, width: proof.width() };
    let skip = skip_guard(&mut seeds, proof).expect("pre-tested");
    for (phi, operands) in leaving(&mut seeds, &replacement, true) {
        seeds.function.set_operands(phi, operands);
    }
    let void = seeds.function.instruction(jump).ty;
    let branch = seeds
        .function
        .create_instruction(
            Opcode::Br,
            void,
            vec![Operand::Value(skip), Operand::Block(exit), Operand::Block(header)],
            Flags::default(),
            None,
        );
    seeds.function.insert(branch, llrm_mir::edit::Position::Before(jump)).unwrap();
    seeds.function.erase(jump).unwrap();
}

const INPUTS: &[&[i128]] = &[&[5, 5], &[5, 6], &[-3, 40], &[7, 2], &[-32768, -32768]];

#[test]
fn a_symbolic_exit_value_and_skip_test_are_placed_before_the_loop() {
    let before = parsed(&counting("%s", "%n"));
    let mut module = before.clone();
    guarded(&mut module);
    let text = printed(&module);
    assert!(
        text.contains("b0:\n  %0 = icmp sle i16 %n, %s\n  %1 = add i16 %n, 0\n  br i1 %0, label %b3, label %b1\n")
            && text.contains("b3:\n  %e = phi i16 [ %1, %b1 ], [ %s, %b0 ]\n"),
        "{text}"
    );
    assert_eq!(results(&module, INPUTS), results(&before, INPUTS));
}

/// Known start and bound give a known exit value: nothing is placed for it.
#[test]
fn a_constant_exit_value_places_nothing() {
    for bound in ["0", "1", "40"] {
        let before = parsed(&counting("0", bound));
        let mut module = before.clone();
        guarded(&mut module);
        let text = printed(&module);
        assert!(text.contains(&format!("b0:\n  %0 = icmp sle i16 {bound}, 0\n  br i1 %0")), "{text}");
        assert!(text.contains(&format!("%e = phi i16 [ {bound}, %b1 ], [ 0, %b0 ]")), "{text}");
        assert_eq!(results(&module, INPUTS), results(&before, INPUTS));
    }
}
