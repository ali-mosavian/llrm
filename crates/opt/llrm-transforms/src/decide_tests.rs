//! Adapted from llrm-codegen's `optimize/transform_tests.rs` (`d_tests`), each
//! body now MIR text run by llrm-mir's interpreter.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::module::Module;
use llrm_mir::passes::Outer;

use super::{Decide, decided};
use crate::testing::{ACROSS_READONLY_CALL, managed, parsed, printed, results};

/// @f decided; whether anything changed.
fn decide(module: &mut Module) -> bool {
    let (layout, outer) = (layout(module), Outer::of(module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    decided(context, &layout, function, &outer).expect("decides")
}

/// @f through JumpThreading, which threads and settles what `decided` once did.
fn jump_threaded(module: &mut Module) {
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(crate::jumpthreading::JumpThreading { enabled: true, options: Default::default() });
    manager.run_module(module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
}

/// @f decided by the guard engine (ScalarEvolution is not asked).
fn decide_by_guards(module: &mut Module) -> bool {
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

/// A branch on a compare of two numbers is a jump the way it goes, and
/// the other arm goes.
#[test]
fn test_a_branch_on_known_numbers_is_a_jump() {
    check(
        "define i16 @f(i16 %x) {
b0:
  %c = icmp slt i16 5, 10
  br i1 %c, label %b1, label %b2

b1:
  ret i16 %x

b2:
  ret i16 0
}
",
        "define i16 @f(i16 %x) {
b0:
  %c = icmp slt i16 5, 10
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

/// A switch on a constant is a jump to its case (the constant is folded into
/// the operand, as the pin's IRBuilder leaves it).
#[test]
fn test_a_switch_on_a_known_number_is_a_jump_to_its_case() {
    check(
        "define i16 @f(i16 %x) {
b0:
  switch i16 2, label %d [ i16 1, label %a
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
        managed(&mut module, Decide { constants: true }),
        "define i16 @f(i16 %x) {
b0:
  %c = icmp eq i16 1, 1
  ret i16 %x
}
"
    );
}

/// A number kept in a cell across a readonly call is put in the branch's
/// condition by `fold` (it reads the memory), and `decide` then takes the
/// branch: the pair decides it, as the pipeline runs them.
#[test]
fn a_branch_on_a_cell_kept_across_a_readonly_call_is_decided_after_fold() {
    let text = ACROSS_READONLY_CALL.replace(
        "  ret i16 %v\n",
        "  %c = icmp eq i16 %v, 7\n  br i1 %c, label %b1, label %b2\n\nb1:\n  ret i16 1\n\nb2:\n  ret i16 2\n",
    );
    let mut module = parsed(&format!("{DOS}{text}"));
    managed(&mut module, crate::fold::Fold);
    let after = managed(&mut module, Decide { constants: true });
    assert!(!after.contains("br i1"), "{after}");
}

/// Decide solved memory for the body (a quarter of the pass, 33 M of d_faces
/// -O2's 2,856 M) and decided none of 574 branches by it: a number memory gave
/// is `fold`'s to put in the condition. It asks the manager for the values
/// alone.
#[test]
fn decide_solves_no_memory() {
    let text = ACROSS_READONLY_CALL.replace(
        "  ret i16 %v\n",
        "  %c = icmp eq i16 %v, 7\n  br i1 %c, label %b1, label %b2\n\nb1:\n  ret i16 1\n\nb2:\n  ret i16 2\n",
    );
    let mut module = parsed(&format!("{DOS}{text}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let mut analyses = llrm_mir::passes::Analyses::new(std::rc::Rc::new(outer));
    super::_decided(context, &layout, function, &mut analyses, true).expect("decides");
    assert!(analyses.cached::<llrm_analysis::manager::ThroughMemory>().is_none(), "decide solved memory for the body");
}

/// The facts decide asked of the manager beside the values were the memory
/// annotations and points-to of the whole body (26.0 M of d_faces -O2); across
/// the corpus (1,580 compiles, 33,932 runs, 3,002 decisions) a decision made
/// without them is the same decision, none lost, none new.
#[test]
fn decide_asks_no_memory_annotations_or_points_to() {
    let text = ACROSS_READONLY_CALL.replace(
        "  ret i16 %v\n",
        "  %c = icmp eq i16 %v, 7\n  br i1 %c, label %b1, label %b2\n\nb1:\n  ret i16 1\n\nb2:\n  ret i16 2\n",
    );
    let mut module = parsed(&format!("{DOS}{text}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let mut analyses = llrm_mir::passes::Analyses::new(std::rc::Rc::new(outer));
    super::_decided(context, &layout, function, &mut analyses, true).expect("decides");
    assert!(analyses.cached::<llrm_analysis::manager::PointerValues>().is_none(), "decide solved points-to");
    assert!(analyses.cached::<llrm_analysis::manager::Annotated>().is_none(), "decide annotated the body's memory");
}

/// Whether @f still has a block named `name`.
fn has(
    module: &Module,
    name: &str,
) -> bool {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    function.layout().iter().any(|&one| function.block(one).name.as_deref() == Some(name))
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
    jump_threaded(&mut module);
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

/// ScalarEvolution does not bound that index by the guards: it is not an add
/// recurrence (the counter steps it in one arm only), as at the pin, where
/// `opt` keeps this check through `simplifycfg`, `jump-threading` and
/// `correlated-propagation` and drops it after `loop-rotate`, `indvars` and
/// `simplifycfg`. The guard engine behind it decides it, so switching to
/// ScalarEvolution loses nothing.
#[test]
fn test_an_index_that_follows_the_counter_is_decided_by_the_guards() {
    let mut module = parsed(&format!("{DOS}{PARTITION_CHECK}"));
    let before = results(&module, INPUTS);
    assert!(decide_by_guards(&mut module));
    assert!(!printed(&module).contains("br i1 %i_ok"), "{}", printed(&module));
    assert_eq!(results(&module, INPUTS), before);
}

/// The loop-exit check of a counted loop (`rectwo`'s `_paths`, seven deep:
/// -O2 and -O3 +10% clocks with ScalarEvolution on before LazyValueInfo
/// settled branches). `%k` is the loop's count,
/// the loop is entered only with `%n != 0`, and it continues only with
/// `%k - 1 != 0`, so `%k` is not zero where the loop is left from its header.
/// The old engine decides it, and so does LazyValueInfo (the pinned `opt` drops
/// it in `correlated-propagation`); ScalarEvolution has no fact about `%k`.
const COUNT_NOT_ZERO_AT_THE_EXIT: &str = "define i16 @f(i16 %inv, i16 %n) {
entry:
  %z0 = icmp eq i16 %n, 0
  br i1 %z0, label %zero, label %pre
pre:
  br label %h
h:
  %c = phi i16 [ 0, %pre ], [ %c2, %l ]
  %k = phi i16 [ %n, %pre ], [ %dec, %l ]
  %t = icmp eq i16 %inv, 1
  br i1 %t, label %l, label %x
l:
  %dec = sub nsw i16 %k, 1
  %c2 = add i16 %c, 1
  %d = icmp eq i16 %dec, 0
  br i1 %d, label %out, label %h
x:
  %z = icmp eq i16 %k, 0
  br i1 %z, label %zero, label %nz
out:
  ret i16 %c2
nz:
  ret i16 7
zero:
  ret i16 0
}
";

#[test]
fn test_a_count_checked_nonzero_before_its_loop_is_nonzero_at_the_loops_exit_by_the_old_engine() {
    let mut module = parsed(&format!("{DOS}{COUNT_NOT_ZERO_AT_THE_EXIT}"));
    let before = results(&module, &[&[1, 3], &[0, 3], &[1, 0], &[0, 0]]);
    assert!(printed(&module).contains("br i1 %z,"), "premise: the check is in the body");
    assert!(decide_by_guards(&mut module));
    assert!(!printed(&module).contains("br i1 %z,"), "{}", printed(&module));
    assert_eq!(results(&module, &[&[1, 3], &[0, 3], &[1, 0], &[0, 0]]), before);
}

#[test]
fn test_the_count_check_at_the_loops_exit_is_decided_by_the_guards() {
    let mut module = parsed(&format!("{DOS}{COUNT_NOT_ZERO_AT_THE_EXIT}"));
    assert!(decide_by_guards(&mut module));
    assert!(!printed(&module).contains("br i1 %z,"), "{}", printed(&module));
}

/// A condition a dominating branch implies is decided by the guards:
/// `n > 5` taken, `n > 3` is true.
#[test]
fn test_a_compare_a_dominating_branch_implies_is_decided_by_the_guards() {
    let text = "define i16 @f(i16 %n) {
b:
  %big = icmp sgt i16 %n, 5
  br i1 %big, label %in, label %out
in:
  %mid = icmp sgt i16 %n, 3
  br i1 %mid, label %yes, label %no
yes:
  ret i16 1
no:
  ret i16 0
out:
  ret i16 2
}
";
    let mut module = parsed(&format!("{DOS}{text}"));
    assert!(decide_by_guards(&mut module));
    assert!(!printed(&module).contains("br i1 %mid"), "{}", printed(&module));
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

/// `zip(a, b)` bounded by both slices' lengths: the header copy's phi takes
/// `%10` from `b25`, the edge where `%10` is false, so it is `false` there and
/// the branch on the phi is threaded. Only a literal arm was a constant
/// to `decide`: the arm became one when `gvn` rewrote it, a round later, and
/// the nib test
/// `test_a_zip_loop_tested_for_equality_ends_on_the_flags_of_its_byte_counter`
/// needed a fourth round of the pipeline (and failed under a budget of three).
/// One run decides it.
#[test]
fn test_a_run_of_decide_leaves_nothing_for_another() {
    let text = "define i32 @f(ptr addrspace(1) %0, ptr addrspace(1) %1) {
b1:
  %2 = load i16, ptr addrspace(1) %0
  %3 = load i16, ptr addrspace(1) %1
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %5 = load ptr addrspace(1), ptr addrspace(1) %4
  %6 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %7 = load ptr addrspace(1), ptr addrspace(1) %6
  br label %b3

b3:
  %8 = phi i16 [ 0, %b1 ], [ %24, %b11 ]
  %9 = phi i32 [ 0, %b1 ], [ %23, %b11 ]
  %10 = icmp ult i16 %8, %2
  br i1 %10, label %b6, label %b25

b4:
  br i1 %10, label %b9, label %b10

b5:
  %11 = phi i32 [ %9, %b7 ]
  ret i32 %11

b6:
  %12 = icmp ult i16 %8, %3
  br label %b7

b7:
  %13 = phi i1 [ %10, %b25 ], [ %12, %b6 ]
  br i1 %13, label %b4, label %b5

b9:
  %14 = mul i16 %8, 2
  %15 = getelementptr inbounds i8, ptr addrspace(1) %5, i16 %14
  %16 = icmp ult i16 %8, %3
  br i1 %16, label %b11, label %b12

b10:
  unreachable

b11:
  %17 = getelementptr inbounds i8, ptr addrspace(1) %7, i16 %14
  %18 = load i16, ptr addrspace(1) %15
  %19 = sext i16 %18 to i32
  %20 = load i16, ptr addrspace(1) %17
  %21 = sext i16 %20 to i32
  %22 = mul i32 %19, %21
  %23 = add i32 %9, %22
  %24 = add i16 %8, 1
  br label %b3

b12:
  unreachable

b25:
  br label %b7
}
";
    let mut module = parsed(text);
    jump_threaded(&mut module);
    managed(&mut module, Decide { constants: true });
    assert!(!decide(&mut module), "a run left a decision for the next:\n{}", printed(&module));
    assert!(!printed(&module).contains("phi i1 ["), "the branch on the phi was threaded:\n{}", printed(&module));
}

/// A branch on `x u< 20` after `llvm.assume(x u< 10)` is always taken: the
/// assume is LazyValueInfo's to read (`getPredicateAt`), not `ranges`'. It
/// stayed a branch.
#[test]
fn a_branch_an_assume_settles_is_decided() {
    let mut module = parsed(
        "declare void @llvm.assume(i1)

define i16 @f(i16 %x) {
b0:
  %a = icmp ult i16 %x, 10
  call void @llvm.assume(i1 %a)
  %c = icmp ult i16 %x, 20
  br i1 %c, label %b1, label %b2

b1:
  ret i16 1

b2:
  ret i16 2
}
",
    );
    jump_threaded(&mut module);
    assert!(!has(&module, "b2"), "{}", printed(&module));
}

/// `br i1 %c, T, T` decided left the two inputs `T`'s phi had from the block
/// for the one edge a jump has: the verifier refused ("2 inputs from block
/// for 1 edges") after JumpThreading made such a branch (QCport `sc_shift`).
#[test]
fn test_a_branch_to_one_block_twice_leaves_its_phi_one_input() {
    let mut module = parsed(
        "define i16 @f(i16 %x) {
b0:
  %c = icmp slt i16 2, 3
  br i1 %c, label %b1, label %b1

b1:
  %p = phi i16 [ %x, %b0 ], [ %x, %b0 ]
  ret i16 %p
}
",
    );
    assert!(decide(&mut module));
    assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new(), "{}", printed(&module));
}

/// In LLVM's order `Decide` leaves the numbers to SimplifyCFG, CVP and SCCP
/// (no row of the 71 programs needed them here) and keeps what the guards and
/// ScalarEvolution prove: constants off, a switch on a known number stays and
/// a compare the dominating edge settles goes.
#[test]
fn test_without_constants_decide_keeps_the_implied_branches_only() {
    let mut switch = parsed(
        "define i16 @f(i16 %x) {\nb0:\n  %s = add i16 1, 1\n  switch i16 %s, label %d [ i16 1, label %a\n                            i16 2, label %b ]\na:\n  ret i16 10\nb:\n  ret i16 %x\nd:\n  ret i16 30\n}\n",
    );
    let before = printed(&switch);
    managed(&mut switch, Decide { constants: false });
    assert_eq!(printed(&switch), before, "a switch on a known number is not Decide's here");
    let mut implied = parsed(
        "define i16 @f(i16 %x) {\nb0:\n  %c = icmp slt i16 %x, 5\n  br i1 %c, label %b1, label %b3\nb1:\n  %d = icmp slt i16 %x, 10\n  br i1 %d, label %b2, label %b3\nb2:\n  ret i16 1\nb3:\n  ret i16 0\n}\n",
    );
    managed(&mut implied, Decide { constants: false });
    assert!(!printed(&implied).contains("br i1 %d"), "{}", printed(&implied));
}

/// Each ask for a function's loop-form blocks built the whole body's graph and
/// a predecessor map of it (d_faces -O2: 33 asks, 2.9 M Ir) to read the few
/// blocks round its loops. With the loops held, it builds no graph, and finds
/// the preheader `b0` and the latch `b2`.
#[test]
fn test_the_loop_form_blocks_are_read_without_the_whole_graph() {
    let mut module = parsed(
        "define void @f(i16 %n) {
b0:
  br label %b1

b1:
  %c = icmp slt i16 %n, 0
  br i1 %c, label %b2, label %b3

b2:
  br label %b1

b3:
  ret void
}
",
    );
    let (_, function) = module.function_mut("f").expect("@f");
    let function = &*function;
    let loops = llrm_analysis::cfg::loops_of(function);
    let before = llrm_analysis::cfg::graphs_built();
    let found = super::loop_form_blocks(function, Some(loops));
    assert_eq!(llrm_analysis::cfg::graphs_built() - before, 0, "the whole graph was built");
    assert_eq!(found.into_iter().collect::<Vec<_>>(), [0, 2]);
}

/// Every nested loop's `Around::exits` read the successors of its whole body
/// again (d_faces -O2: 188 loops, 1.5 M Ir). The blocks round the loops are
/// the ones `exits`-based reading finds, with no exit scan at all.
#[test]
fn test_the_loop_form_blocks_of_nested_loops_need_no_exit_scan() {
    use llrm_analysis::cfg::{Around, predecessors_of, successors_of};
    use llrm_mir::counts::{Event, get};
    let mut module = parsed(
        "define void @f(i16 %n) {
b0:
  br label %b1

b1:
  %c = icmp slt i16 %n, 0
  br i1 %c, label %b2, label %b7

b2:
  br label %b3

b3:
  %d = icmp slt i16 %n, 5
  br i1 %d, label %b4, label %b5

b4:
  br label %b3

b5:
  %e = icmp slt i16 %n, 9
  br i1 %e, label %b6, label %b1

b6:
  br label %b7

b7:
  ret void
}
",
    );
    let (_, function) = module.function_mut("f").expect("@f");
    let function = &*function;
    let reference = {
        let mut edges = std::collections::BTreeSet::new();
        for loop_ in llrm_analysis::cfg::loops_of(function) {
            edges.extend(loop_.preheader(function));
            if let Some(&latch) = loop_.latches.iter().next().filter(|_| loop_.latches.len() == 1)
                && successors_of(function, latch)[..] == [loop_.header]
            {
                edges.insert(latch);
            }
            for (_, exit) in loop_.exits(function) {
                let after = successors_of(function, exit);
                if predecessors_of(function, exit).iter().all(|at| loop_.body.contains(at))
                    && after.len() == 1
                    && !loop_.body.contains(&after[0])
                {
                    edges.insert(exit);
                }
            }
        }
        edges
    };
    assert!(!reference.is_empty());
    let loops = llrm_analysis::cfg::loops_of(function);
    let before = get(Event::LoopExitScans);
    let found = super::loop_form_blocks(function, Some(loops));
    assert_eq!(get(Event::LoopExitScans) - before, 0, "an exit scan was made");
    assert_eq!(found, reference);
}
