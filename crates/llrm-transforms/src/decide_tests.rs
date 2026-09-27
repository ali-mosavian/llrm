//! Adapted from llrm-core's `optimize/transform_tests.rs` (`d_tests`), each
//! body now MIR text run by llrm-mir's interpreter.

use llrm_analysis::testing::layout;
use llrm_mir::module::Module;
use llrm_mir::passes::ModulePass;

use super::{_threaded, Decide, decided};
use crate::testing::{parsed, printed, results};

/// @f decided; whether anything changed.
fn decide(module: &mut Module) -> bool {
    let (layout, id) = (layout(module), module.named("f").expect("@f"));
    decided(module, &layout, id).expect("decides")
}

/// `text` decided is `expected`, and computes what it did on `inputs`.
fn check(text: &str, expected: &str, inputs: &[&[i128]]) {
    let mut module = parsed(text);
    let before = results(&module, inputs);
    assert_eq!(decide(&mut module), text != expected, "{text}");
    assert_eq!(printed(&module), expected);
    assert_eq!(results(&module, inputs), before);
}

/// Collapsed FPCSE's trampoline is removable; a phi edge, a store or a
/// cycle keeps it.
#[test]
fn test_empty_jump_threading_preserves_phi_inputs_and_effects() {
    let body = |middle: &str, end: &str| {
        format!(
            "define i16 @f(i16 %x) {{
b0:
  %a = alloca i16
  br label %b1

b1:
{middle}
b2:
{end}  ret i16 %x
}}
"
        )
    };
    let jump = "  br label %b2\n";
    let phi = "  %r = phi i16 [ %x, %b1 ]\n";
    for (guard, text) in [
        ("none", body(jump, "")),
        ("phi", body(jump, phi)),
        ("store", body(&format!("  store i16 %x, ptr %a\n{jump}"), "")),
        ("cycle", body("  br label %b1\n", "")),
    ] {
        let mut module = parsed(&text);
        let (context, function) = module.function_mut("f").expect("@f");
        let changed = _threaded(context, function);
        assert_eq!(changed, guard == "none", "{guard}");
        let expected = if guard == "none" {
            "define i16 @f(i16 %x) {
b0:
  %a = alloca i16
  br label %b2

b2:
  ret i16 %x
}
"
        } else {
            &text
        };
        assert_eq!(printed(&module), expected, "{guard}");
    }
}

/// A branch on a compare of two numbers is a jump the way it goes, and
/// the other arm goes.
#[test]
fn test_a_branch_on_known_numbers_is_a_jump() {
    check(
        "define i16 @f(i16 %x) {
b0:
  %k = add i16 2, 3
  %c = icmp slt i16 %k, 10
  br i1 %c, label %b1, label %b2

b1:
  ret i16 %x

b2:
  ret i16 0
}
",
        "define i16 @f(i16 %x) {
b0:
  %k = add i16 2, 3
  %c = icmp slt i16 %k, 10
  br label %b1

b1:
  ret i16 %x
}
",
        &[&[4], &[-4]],
    );
}

/// Not taken, the branch is a jump to its other arm, and the join the
/// taken arm fed is the one value left.
#[test]
fn test_a_branch_not_taken_leaves_a_join_its_other_value() {
    check(
        "define i16 @f(i16 %x) {
b0:
  %c = icmp ugt i16 3, 65535
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %r = phi i16 [ 1, %b1 ], [ 2, %b2 ]
  %s = add i16 %r, %x
  ret i16 %s
}
",
        "define i16 @f(i16 %x) {
b0:
  %c = icmp ugt i16 3, -1
  br label %b2

b2:
  br label %b3

b3:
  %s = add i16 2, %x
  ret i16 %s
}
",
        &[&[0], &[9]],
    );
}

/// A switch on a known number is a jump to its case.
#[test]
fn test_a_switch_on_a_known_number_is_a_jump_to_its_case() {
    check(
        "define i16 @f(i16 %x) {
b0:
  %s = add i16 1, 1
  switch i16 %s, label %d [ i16 1, label %a
                            i16 2, label %b ]

a:
  ret i16 10

b:
  ret i16 %x

d:
  ret i16 30
}
",
        "define i16 @f(i16 %x) {
b0:
  %s = add i16 1, 1
  br label %b

b:
  ret i16 %x
}
",
        &[&[1], &[2]],
    );
}

/// A flag only a branch that never runs could set is still clear: the
/// branch reading it is decided along the edges that run.
#[test]
fn test_a_condition_known_along_the_edges_that_run_is_decided() {
    check(
        "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b3 ]
  %flag = phi i16 [ 0, %b0 ], [ %flag2, %b3 ]
  %c = icmp eq i16 %flag, 0
  br i1 %c, label %b3, label %b2

b2:
  br label %b3

b3:
  %flag2 = phi i16 [ %flag, %b1 ], [ 1, %b2 ]
  %next = add i16 %i, 1
  %more = icmp slt i16 %next, %n
  br i1 %more, label %b1, label %b4

b4:
  ret i16 %flag
}
",
        "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b3 ]
  %c = icmp eq i16 0, 0
  br label %b3

b3:
  %next = add i16 %i, 1
  %more = icmp slt i16 %next, %n
  br i1 %more, label %b1, label %b4

b4:
  ret i16 0
}
",
        &[&[0], &[1], &[5]],
    );
}

/// Under `x < 5`, `x < 10` holds: the dominating edge decides it.
#[test]
fn test_a_compare_a_dominating_edge_settles_is_decided() {
    check(
        "define i16 @f(i16 %x) {
b0:
  %c = icmp slt i16 %x, 5
  br i1 %c, label %b1, label %b3

b1:
  %d = icmp slt i16 %x, 10
  br i1 %d, label %b2, label %b3

b2:
  ret i16 1

b3:
  ret i16 0
}
",
        "define i16 @f(i16 %x) {
b0:
  %c = icmp slt i16 %x, 5
  br i1 %c, label %b1, label %b3

b1:
  %d = icmp slt i16 %x, 10
  br label %b2

b2:
  ret i16 1

b3:
  ret i16 0
}
",
        &[&[4], &[5], &[9], &[10], &[-32768]],
    );
}

/// A branch or a switch on what nothing knows stays.
#[test]
fn test_a_branch_on_an_unknown_condition_stays() {
    let text = "define i16 @f(i16 %x) {
b0:
  %c = icmp slt i16 %x, 5
  br i1 %c, label %b1, label %b2

b1:
  switch i16 %x, label %b2 [
    i16 1, label %b3
  ]

b2:
  ret i16 0

b3:
  ret i16 1
}
";
    check(text, text, &[&[1], &[4], &[7]]);
}

/// The pass merges the line a decided branch leaves.
#[test]
fn test_decide_merges_the_chain_it_leaves() {
    let mut module = parsed(
        "define i16 @f(i16 %x) {
b0:
  %c = icmp eq i16 1, 1
  br i1 %c, label %b1, label %b2

b1:
  ret i16 %x

b2:
  ret i16 0
}
",
    );
    assert_eq!(Decide.run(&mut module), vec![module.named("f").unwrap()]);
    assert_eq!(
        printed(&module),
        "define i16 @f(i16 %x) {
b0:
  %c = icmp eq i16 1, 1
  ret i16 %x
}
"
    );
}
