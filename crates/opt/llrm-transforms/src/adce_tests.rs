//! Each body is MIR text run by llrm-mir's interpreter before and after; the
//! expected text is what `opt -passes=adce` makes of the same body (checked
//! with `tools/difftest`).

use llrm_mir::module::Module;

use super::aggressive_dead_code_elimination;
use crate::testing::{parsed, printed, results};

/// @f after ADCE; whether anything changed.
fn adced(module: &mut Module) -> bool {
    let callees = llrm_mir::memory::callees(module);
    let (context, function) = module.function_mut("f").expect("@f");
    aggressive_dead_code_elimination(context, &callees, function).anything
}

/// `text` after ADCE is `expected` and computes what it did on `inputs`.
fn check(
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
    let mut module = parsed(text);
    let before = results(&module, inputs);
    assert_eq!(adced(&mut module), text != expected);
    assert_eq!(printed(&module), expected);
    assert_eq!(results(&module, inputs), before);
}

/// A branch that only chooses whether to compute a dead value becomes a jump:
/// `dead` keeps it, since it feeds a branch.
#[test]
fn test_a_branch_over_dead_work_becomes_a_jump() {
    check(
        "define i16 @f(i1 %c, i16 %x) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %d = add i16 %x, 1
  br label %b2

b2:
  ret i16 %x
}
",
        "define i16 @f(i1 %c, i16 %x) {
b0:
  br label %b2

b1:
  br label %b2

b2:
  ret i16 %x
}
",
        &[&[0, 5], &[1, 5]],
    );
}

/// A branch that decides a live phi stays.
#[test]
fn test_a_branch_that_decides_a_live_phi_stays() {
    let text = "define i16 @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %r = phi i16 [ 1, %b1 ], [ 2, %b2 ]
  ret i16 %r
}
";
    check(text, text, &[&[0], &[1]]);
}

/// A loop that computes nothing anyone reads keeps its back edge: it might not
/// end (`adce-remove-loops` is off), and the sum goes.
#[test]
fn test_a_loop_keeps_its_back_edge_and_loses_its_dead_sum() {
    check(
        "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %j, %b1 ]
  %s = phi i16 [ 0, %b0 ], [ %t, %b1 ]
  %t = add i16 %s, %i
  %j = add i16 %i, 1
  %c = icmp slt i16 %j, %n
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %n
}
",
        "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %j, %b1 ]
  %j = add i16 %i, 1
  %c = icmp slt i16 %j, %n
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %n
}
",
        &[&[1], &[5]],
    );
}

/// A dead switch whose three edges all reach one block: the phi there has an
/// input per edge, the branch keeps the first edge and the other two inputs
/// go, which leaves the phi one value.
#[test]
fn test_a_dead_switch_with_repeated_targets_leaves_one_input_per_kept_edge() {
    check(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  switch i16 %x, label %b1 [
    i16 1, label %b1
    i16 2, label %b1
  ]

b1:
  %r = phi i16 [ %y, %b0 ], [ %y, %b0 ], [ %y, %b0 ]
  ret i16 %r
}
",
        "define i16 @f(i16 %x, i16 %y) {
b0:
  br label %b1

b1:
  ret i16 %y
}
",
        &[&[1, 5], &[3, 5]],
    );
}
