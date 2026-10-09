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
fn check(
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
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
    let text = ACROSS_READONLY_CALL.replace(
        "  ret i16 %v\n",
        "  %c = icmp eq i16 %v, 7\n  br i1 %c, label %b1, label %b2\n\nb1:\n  ret i16 1\n\nb2:\n  ret i16 2\n",
    );
    let mut module = parsed(&format!("{DOS}{text}"));
    let after = managed(&mut module, Decide);
    assert!(!after.contains("br i1"), "{after}");
}

/// Whether @f still has a block named `name`.
fn has(
    module: &Module,
    name: &str,
) -> bool {
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

/// A pointer to an object compared with null: never equal. A parameter may be
/// null.
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

/// A near parameter the language says is dereferenceable (QB's BYREF: the
/// address of a variable) or non-null is never null; a far one may be 0000:0000
/// and a plain one may be anything (#113).
#[test]
fn a_dereferenceable_near_parameter_is_never_null() {
    for (attributes, parameter, decided) in [
        ("dereferenceable(2)", "ptr", true),
        ("nonnull", "ptr", true),
        ("", "ptr", false),
        ("dereferenceable(2)", "ptr addrspace(1)", false),
    ] {
        let mut module = parsed(&format!(
            "define i16 @f({parameter} {attributes} %p) {{
b0:
  %c = icmp eq {parameter} %p, null
  br i1 %c, label %yes, label %no

yes:
  ret i16 1

no:
  ret i16 2
}}
"
        ));
        decide(&mut module);
        assert_eq!(!has(&module, "yes"), decided, "{parameter} {attributes}: {}", printed(&module));
    }
}

/// An `&&`'s false edge reached a phi only to branch on it again: N$PQ4's
/// loop exit tested a byte it had just set.
#[test]
fn an_edge_giving_a_branch_phi_a_constant_goes_where_it_leads() {
    let text = "define i16 @f(i16 %x, i16 %y) {
b0:
  %a = icmp eq i16 %x, 0
  br i1 %a, label %b1, label %b5

b5:
  br label %b2

b1:
  %b = icmp eq i16 %y, 0
  br label %b2

b2:
  %c = phi i1 [ false, %b5 ], [ %b, %b1 ]
  br i1 %c, label %b3, label %b4

b3:
  ret i16 1

b4:
  ret i16 0
}
";
    let inputs: &[&[i128]] = &[&[0, 0], &[0, 1], &[1, 0], &[1, 1]];
    let mut module = parsed(text);
    let before = results(&module, inputs);
    assert!(decide(&mut module));
    let after = printed(&module);
    assert!(!after.contains("phi"), "{after}");
    assert_eq!(results(&module, inputs), before);
}

/// A pointer parameter stated `nonnull` is never null: compared with it the
/// branch is decided; unstated, it is not.
#[test]
fn a_nonnull_parameter_is_never_null() {
    for (attribute, decided) in [("nonnull", true), ("", false)] {
        let mut module = parsed(&format!(
            "define i16 @f(ptr {attribute} %p) {{
b0:
  %c = icmp eq ptr %p, null
  br i1 %c, label %yes, label %no

yes:
  ret i16 1

no:
  ret i16 0
}}
"
        ));
        decide(&mut module);
        assert_eq!(!has(&module, "yes"), decided, "{attribute}: {}", printed(&module));
    }
}

/// quicksort's `a[i]` in the swap, `i` the partition index that follows the
/// counter `j`: with `lo <u len` taken before the loop and `j < hi < len`, `i`
/// is below `len`, and its check was kept, 3 instructions on each swap.
const PARTITION_CHECK: &str = "@cell = global i16 10

define i16 @f(i16 %lo, i16 %hi, i16 %pivot) {
entry:
  %len = load i16, ptr @cell, !range !0
  %ordered = icmp slt i16 %lo, %hi
  br i1 %ordered, label %first, label %done
first:
  %hi_ok = icmp ult i16 %hi, %len
  %lo_ok = icmp ult i16 %lo, %len
  %both = and i1 %hi_ok, %lo_ok
  br i1 %both, label %head, label %crash
head:
  %j = phi i16 [ %lo, %first ], [ %next, %join ]
  %i = phi i16 [ %lo, %first ], [ %kept, %join ]
  %more = icmp slt i16 %j, %hi
  br i1 %more, label %body, label %done
body:
  %small = icmp slt i16 %j, %pivot
  br i1 %small, label %check, label %join
check:
  %i_ok = icmp ult i16 %i, %len
  br i1 %i_ok, label %step, label %crash
step:
  %up = add i16 %i, 1
  br label %join
join:
  %kept = phi i16 [ %up, %step ], [ %i, %body ]
  %next = add nsw i16 %j, 1
  br label %head
done:
  %r = phi i16 [ 0, %entry ], [ %i, %head ]
  ret i16 %r
crash:
  ret i16 99
}

!0 = !{i16 0, i16 -32768}
";

#[test]
fn test_an_index_that_follows_the_counter_is_below_the_length_the_loop_was_checked_against() {
    let mut module = parsed(&format!("{DOS}{PARTITION_CHECK}"));
    let before = results(&module, INPUTS);
    assert!(printed(&module).contains("br i1 %i_ok"), "premise: the check is in the loop");
    assert!(decide(&mut module));
    let after = printed(&module);
    assert!(!after.contains("br i1 %i_ok"), "{after}");
    assert_eq!(results(&module, INPUTS), before);
}

/// Where the counter may be negative the check stays: `lo` is not known below
/// `len`.
#[test]
fn test_an_index_from_an_unchecked_start_keeps_its_check() {
    let mut module = parsed(&format!(
        "{DOS}{}",
        PARTITION_CHECK.replace("  %both = and i1 %hi_ok, %lo_ok\n  br i1 %both,", "  br i1 %hi_ok,")
    ));
    decide(&mut module);
    assert!(printed(&module).contains("br i1 %i_ok"), "{}", printed(&module));
}

const INPUTS: &[&[i128]] = &[&[0, 5, 3], &[2, 9, 6], &[-1, 4, 2], &[3, 3, 1], &[5, 2, 0], &[0, 9, 20], &[1, 9, -3]];

/// A parameter's stated range settles a compare against a number beyond it:
/// queens' `row` is 0 to 7, its check against a length of 12 never fails.
#[test]
fn test_a_compare_a_parameters_range_settles_is_decided() {
    let text = "define i16 @f(i16 range(i16 0, 8) %row) {
b:
  %fits = icmp ult i16 %row, 12
  br i1 %fits, label %ok, label %crash
ok:
  ret i16 %row
crash:
  ret i16 99
}
";
    let mut module = parsed(&format!("{DOS}{text}"));
    assert!(decide(&mut module));
    assert!(!printed(&module).contains("br i1 %fits"), "{}", printed(&module));
}

/// Every guard query found the body's `llvm.assume`s again, a walk of the whole
/// body naming each call's callee by string (`Intrinsic::named`): decide was
/// 5.9% of QCport, a fifth of it that. The body's assumptions are found once,
/// as LLVM's AssumptionCache.
#[test]
fn test_a_body_is_searched_for_its_assumptions_once_however_many_guards_are_asked() {
    let mut module = parsed(&format!("{DOS}{PARTITION_CHECK}"));
    let before = llrm_analysis::assumptions::built();
    assert!(decide(&mut module));
    let searched = llrm_analysis::assumptions::built() - before;
    assert_eq!(searched, 1, "the body was searched {searched} times");
}

/// `decide` solved what the counted loops bound twice: once for the points-to
/// it holds (the manager's `Bounded`) and again for its own branches (a hand
/// solve): 26 Minstr of fpbench's -O1 compile. It reads the manager's.
#[test]
fn test_decide_works_a_loops_bounds_out_once() {
    let mut module = parsed(
        "define i32 @f(i32 %n) {
b0:
  br label %h

h:
  %i = phi i32 [ 0, %b0 ], [ %in, %l ]
  %c = icmp slt i32 %i, 10
  br i1 %c, label %l, label %end

l:
  %in = add nsw i32 %i, 1
  br label %h

end:
  %t = icmp slt i32 %n, 5
  br i1 %t, label %a, label %b

a:
  ret i32 1

b:
  ret i32 2
}
",
    );
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let before = llrm_analysis::ranges::loops_solved();
    decided(context, &layout, function, &outer).expect("decides");
    assert_eq!(llrm_analysis::ranges::loops_solved() - before, 1, "the loop's bounds were worked out more than once");
}

/// `decide` solved the body through memory a second time (calls assumed to
/// write what the target says), beside the manager's `ThroughMemory` that other
/// passes read: 1.3% of the -O1 compile of QCport, for facts the manager had.
/// It asks the manager's.
#[test]
fn test_decide_reads_what_is_known_through_memory_from_the_manager() {
    use llrm_analysis::manager::ThroughMemory;
    use llrm_mir::passes::Analyses;
    let mut module = parsed(&format!(
        "{DOS}@g = global i16 0\n\ndefine i16 @f(i16 %a) {{\nb0:\n  store i16 3, ptr @g\n  %v = load i16, ptr @g\n  %c = icmp eq i16 %v, 3\n  br i1 %c, label %t, label %e\n\nt:\n  ret i16 %a\n\ne:\n  ret i16 0\n}}\n"
    ));
    let (layout, outer) = (layout(&module), std::rc::Rc::new(Outer::of(&module, None)));
    let (context, function) = module.function_mut("f").expect("@f");
    let mut analyses = Analyses::new(outer);
    analyses.get::<ThroughMemory>(context, &layout, function);
    let before = llrm_analysis::consts::memory_derivations();
    assert!(
        super::_decided(context, &layout, function, &mut analyses).expect("decides"),
        "premise: the branch is decided"
    );
    assert_eq!(
        llrm_analysis::consts::memory_derivations() - before,
        0,
        "decide derived what is known through memory again"
    );
}
