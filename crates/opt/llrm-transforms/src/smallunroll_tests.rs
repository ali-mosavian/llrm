//! gcc's small-loop unroll on loops the interpreter runs, before and after.

use llrm_mir::module::Module;
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::profit::OperationCosts;
use crate::smallunroll::SmallUnroll;
use crate::testing::{Tuned, f, managed_on, parsed, printed, results};

/// A machine that unrolls a loop of `insns` instructions by two.
fn machine(insns: i64) -> Tuned {
    let costs = OperationCosts {
        add: 1,
        branch: 1,
        small_unroll_insns: insns,
        small_unroll_factor: if insns > 0 { 2 } else { 0 },
        ..OperationCosts::default()
    };
    Tuned { costs, ..Tuned::default() }
}

/// `s += i` for `i` below `bound`, a body of two additions and a test: three
/// instructions as gcc counts them, four as it lowers them.
fn sum(bound: &str) -> String {
    format!(
        "define i16 @f(i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i2, %b2 ]
  %s = phi i16 [ 0, %b0 ], [ %s2, %b2 ]
  %go = icmp slt i16 %i, {bound}
  br i1 %go, label %b2, label %b3

b2:
  %s2 = add i16 %s, %i
  %i2 = add i16 %i, 1
  br label %b1

b3:
  ret i16 %s
}}
"
    )
}

const INPUTS: &[&[i128]] = &[&[-3], &[0], &[1], &[2], &[3], &[4], &[5], &[6], &[7], &[8], &[9]];

fn adds(module: &mut Module) -> usize {
    let function = f(module);
    function.walk().filter(|&(_, inst)| function.instruction(inst).opcode == Opcode::Binary(BinaryOp::Add)).count()
}

/// `text` through the pass on `machine`: the additions it ends with, and it
/// computes what it did.
fn through(
    text: &str,
    machine: Tuned,
) -> (usize, usize) {
    let mut module = parsed(text);
    let (before, expected) = (adds(&mut module), results(&module, INPUTS));
    let after = managed_on(&mut module, SmallUnroll, machine);
    assert_eq!(results(&module, INPUTS), expected, "{after}");
    (before, adds(&mut module))
}

#[test]
fn test_an_even_constant_count_chains_two_bodies() {
    let (before, after) = through(&sum("8"), machine(4));
    assert_eq!((before, after), (2, 4));
}

#[test]
fn test_an_odd_constant_count_peels_one_trip_first() {
    let (before, after) = through(&sum("7"), machine(4));
    assert_eq!((before, after), (2, 6));
}

#[test]
fn test_a_runtime_count_peels_its_odd_trip_behind_a_test() {
    let (before, after) = through(&sum("%n"), machine(4));
    assert_eq!((before, after), (2, 6));
}

#[test]
fn test_a_loop_that_rolls_under_four_trips_is_left() {
    assert_eq!(through(&sum("3"), machine(4)), (2, 2));
}

#[test]
fn test_a_loop_over_the_targets_size_is_left() {
    assert_eq!(through(&sum("%n"), machine(2)), (2, 2));
}

#[test]
fn test_a_target_without_the_unroll_leaves_the_loop() {
    assert_eq!(through(&sum("%n"), machine(0)), (2, 2));
}

#[test]
fn test_the_unrolled_loop_tests_once_a_trip_of_two_bodies() {
    let mut module = parsed(&sum("%n"));
    managed_on(&mut module, SmallUnroll, machine(4));
    let text = printed(&module);
    // The parity test, the peeled trip's and the loop's: not one a body.
    assert_eq!(text.matches("br i1").count(), 3, "{text}");
}

/// `s += i` tested at its end, entered behind a test of `n`, as the loops LSR
/// leaves are.
fn tested_at_end(bound: &str) -> String {
    format!(
        "define i16 @f(i16 %n) {{
b0:
  %some = icmp sgt i16 {bound}, 0
  br i1 %some, label %b1, label %b4

b1:
  br label %b2

b2:
  %i = phi i16 [ 0, %b1 ], [ %i2, %b2 ]
  %s = phi i16 [ 0, %b1 ], [ %s2, %b2 ]
  %s2 = add i16 %s, %i
  %i2 = add i16 %i, 1
  %more = icmp slt i16 %i2, {bound}
  br i1 %more, label %b2, label %b3

b3:
  %out = phi i16 [ %s2, %b2 ]
  br label %b4

b4:
  %r = phi i16 [ 0, %b0 ], [ %out, %b3 ]
  ret i16 %r
}}
"
    )
}

#[test]
fn test_a_loop_tested_at_its_end_chains_two_bodies_and_drops_the_first_test() {
    assert_eq!(through(&tested_at_end("8"), machine(4)), (2, 4));
    assert_eq!(through(&tested_at_end("7"), machine(4)), (2, 6));
}

#[test]
fn test_a_runtime_count_of_a_loop_tested_at_its_end_is_made_even() {
    assert_eq!(through(&tested_at_end("%n"), machine(4)), (2, 6));
}
