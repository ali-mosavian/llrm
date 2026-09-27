//! Adapted from llrm-core's `optimize/transform_tests.rs` (`d_tests`), each
//! body now MIR text run by llrm-mir's interpreter.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::module::Module;
use llrm_mir::passes::Outer;

use super::{_threaded, Decide, decided};
use crate::testing::{ACROSS_READONLY_CALL, managed, parsed, printed, results};

/// @f decided; whether anything changed.
fn decide(module: &mut Module) -> bool {
    let (layout, outer) = (layout(module), Outer::of(module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    decided(context, &layout, function, &outer).expect("decides")
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
    assert_eq!(
        managed(&mut module, Decide),
        "define i16 @f(i16 %x) {
b0:
  %c = icmp eq i16 1, 1
  ret i16 %x
}
"
    );
}

/// Under the pass manager Decide still reads @peek's `readonly`: without
/// the module's globals it took the call for a writer and left the branch.
#[test]
fn a_branch_on_a_cell_kept_across_a_readonly_call_is_decided() {
    let text = ACROSS_READONLY_CALL.replace("  ret i16 %v\n", "  %c = icmp eq i16 %v, 7\n  br i1 %c, label %b1, label %b2\n\nb1:\n  ret i16 1\n\nb2:\n  ret i16 2\n");
    let mut module = parsed(&format!("{DOS}{text}"));
    let after = managed(&mut module, Decide);
    assert!(!after.contains("br i1"), "{after}");
}

/// Whether @f still has a block named `name`.
fn has(module: &Module, name: &str) -> bool {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    function.layout().iter().any(|&one| function.block(one).name.as_deref() == Some(name))
}

/// `i < -5` inside `for i = 0 to 9`: only the counted loop's range, not the
/// header's edge, says it never holds.
#[test]
fn a_branch_the_counted_loops_range_rules_out_is_decided() {
    let mut module = parsed(
        "define i16 @f() {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b4 ]
  %s = phi i16 [ 0, %b0 ], [ %t, %b4 ]
  %c = icmp slt i16 %i, 10
  br i1 %c, label %b2, label %b5

b2:
  %neg = icmp slt i16 %i, -5
  br i1 %neg, label %b3, label %b4

b3:
  %u = add i16 %s, 100
  br label %b4

b4:
  %t = phi i16 [ %s, %b2 ], [ %u, %b3 ]
  %next = add i16 %i, 1
  br label %b1

b5:
  ret i16 %s
}
",
    );
    let before = results(&module, &[&[]]);
    decide(&mut module);
    assert!(!has(&module, "b3"), "{}", printed(&module));
    assert_eq!(results(&module, &[&[]]), before);
}

/// A pointer to an object compared with null: never equal. A parameter may be null.
#[test]
fn a_pointer_to_an_object_is_never_null() {
    for (pointer, predicate, decided) in [
        ("%a", "eq", true),
        ("%a", "ne", true),
        ("%q", "eq", true), // an object's, through points-to
        ("%p", "eq", false),
    ] {
        let mut module = parsed(&format!(
            "define i16 @f(ptr %p) {{
b0:
  %a = alloca [4 x i16]
  %q = getelementptr i8, ptr %a, i16 2
  %c = icmp {predicate} ptr {pointer}, null
  br i1 %c, label %yes, label %no

yes:
  ret i16 1

no:
  ret i16 2
}}
"
        ));
        decide(&mut module);
        let gone = if predicate == "eq" { "yes" } else { "no" };
        assert_eq!(!has(&module, gone), decided, "{pointer} {predicate}: {}", printed(&module));
        assert!(has(&module, if predicate == "eq" { "no" } else { "yes" }));
    }
}
