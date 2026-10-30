//! Each body is MIR text run by llrm-mir's interpreter before and after.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{Declared, Outer};

use crate::simplifycfg::{Options, simplified_with};
use crate::testing::{parsed, printed, results};
use crate::twoentryphi::Speculation;

/// `text` simplified is `expected`, and computes what it did on `inputs`.
fn check(
    options: &Options,
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
    let mut module = parsed(text);
    let before = results(&module, inputs);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    assert_eq!(simplified_with(options, context, &spec, &mut declared, function), text != expected, "{text}");
    assert_eq!(printed(&module), expected);
    assert_eq!(results(&module, inputs), before);
}

/// Without `foldTwoEntryPHINode`, which would make a select of the join first.
fn branching(hoist_common_insts: bool) -> Options {
    Options { speculate_blocks: false, hoist_common_insts, ..Default::default() }
}

/// Two arms that are the same `add` and the same jump: the add runs once, in
/// the branching block, and the arms are gone.
#[test]
fn test_arms_that_are_the_same_run_once_before_the_branch() {
    check(
        &branching(false),
        "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %a = add i16 %x, 1
  br label %b3

b2:
  %b = add i16 %x, 1
  br label %b3

b3:
  %p = phi i16 [ %a, %b1 ], [ %b, %b2 ]
  ret i16 %p
}
",
        "define i16 @f(i16 %x, i1 %c) {
b0:
  %a = add i16 %x, 1
  ret i16 %a
}
",
        &[&[3, 1], &[3, 0]],
    );
}

/// Arms that differ in what the join's phi takes: the phi's inputs become one
/// select on the branch's condition (the terminator hoist, `br` case).
#[test]
fn test_identical_jumps_with_different_phi_inputs_make_a_select() {
    check(
        &branching(false),
        "define i16 @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi i16 [ 1, %b1 ], [ 2, %b2 ]
  ret i16 %p
}
",
        "define i16 @f(i1 %c) {
b0:
  %0 = select i1 %c, i16 1, i16 2
  ret i16 %0
}
",
        &[&[1], &[0]],
    );
}

/// Arms that are not all the same stay as they are by default
/// (`HoistCommonInsts` off).
#[test]
fn test_arms_that_differ_stay_without_hoist_common_insts() {
    let text = "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %m = mul i16 %x, 3
  %a = add i16 %x, 1
  br label %b3

b2:
  %n = mul i16 %x, 5
  %b = add i16 %x, 1
  br label %b3

b3:
  %p = phi i16 [ %m, %b1 ], [ %n, %b2 ]
  ret i16 %p
}
";
    check(&branching(false), text, text, &[&[3, 1], &[3, 0]]);
}

/// With `HoistCommonInsts` the same add that follows a different first
/// instruction in each arm runs once, before the branch; the muls stay.
#[test]
fn test_a_common_add_after_a_different_first_instruction_is_hoisted() {
    check(
        &branching(true),
        "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %m = mul i16 %x, 3
  %a = add i16 %x, 1
  br label %b3

b2:
  %n = mul i16 %x, 5
  %b = add i16 %x, 1
  br label %b3

b3:
  %p = phi i16 [ %m, %b1 ], [ %n, %b2 ]
  %q = phi i16 [ %a, %b1 ], [ %b, %b2 ]
  %r = add i16 %p, %q
  ret i16 %r
}
",
        "define i16 @f(i16 %x, i1 %c) {
b0:
  %a = add i16 %x, 1
  br i1 %c, label %b1, label %b2

b1:
  %m = mul i16 %x, 3
  br label %b3

b2:
  %n = mul i16 %x, 5
  br label %b3

b3:
  %p = phi i16 [ %m, %b1 ], [ %n, %b2 ]
  %q = phi i16 [ %a, %b1 ], [ %a, %b2 ]
  %r = add i16 %p, %q
  ret i16 %r
}
",
        &[&[3, 1], &[3, 0]],
    );
}

/// A store does not move over a load the arm skipped.
#[test]
fn test_a_store_does_not_move_over_a_skipped_load() {
    let text = "define i16 @f(ptr %q, ptr %r, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %l = load i16, ptr %r
  store i16 7, ptr %q
  br label %b3

b2:
  %k = load i16, ptr %q
  store i16 7, ptr %q
  br label %b3

b3:
  %p = phi i16 [ %l, %b1 ], [ %k, %b2 ]
  ret i16 %p
}
";
    let mut module = parsed(text);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    assert!(!simplified_with(&branching(true), context, &spec, &mut declared, function));
    assert_eq!(printed(&module), text);
}

/// Arms that end differently collected both bodies before the all-equal test
/// said no: the early-out on the terminators takes none.
#[test]
fn test_arms_that_end_differently_are_not_collected_without_hoist_common_insts() {
    let text = "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %a = add i16 %x, 1
  ret i16 %a

b2:
  %b = add i16 %x, 1
  ret i16 %b
}
";
    let mut module = parsed(text);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let (context, function) = module.function_mut("f").expect("@f");
    let before = crate::hoistcommon::BODY_COLLECTS.with(|count| count.get());
    let branch = function.terminator(function.entry().expect("an entry")).expect("a branch");
    assert!(!crate::hoistcommon::hoisted(context, &spec, function, branch, true));
    assert_eq!(crate::hoistcommon::BODY_COLLECTS.with(|count| count.get()), before);
}

/// Two arms making an array's own segment, each with its own `!llrm.descriptor`
/// node, were hoisted into one `inttoptr` without the mark; the module then
/// forged a data-segment pointer, and globals-aa's frozen result was stale
/// (`LLRM_CHECK_STALE` failed bintree.bas -O2 -m16).
#[test]
fn hoisting_two_array_selectors_leaves_the_module_unforged() {
    let mut module = parsed(
        "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %a = inttoptr i16 %x to ptr addrspace(2), !llrm.descriptor !0
  br label %b3

b2:
  %b = inttoptr i16 %x to ptr addrspace(2), !llrm.descriptor !1
  br label %b3

b3:
  %p = phi ptr addrspace(2) [ %a, %b1 ], [ %b, %b2 ]
  %v = load i16, ptr addrspace(2) %p
  ret i16 %v
}

!0 = !{}
!1 = !{!\"x\"}
",
    );
    let spaces = llrm_mir::spaces::Spaces { segment: Some(2), ..llrm_mir::spaces::Spaces::FLAT };
    assert!(!llrm_analysis::forging::forges_selector(&module, spaces));
    let outer = Outer::of(&module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    assert!(simplified_with(&branching(true), context, &spec, &mut declared, function));
    assert_eq!(printed(&module).matches("inttoptr").count(), 1, "{}", printed(&module));
    assert!(!llrm_analysis::forging::forges_selector(&module, spaces), "{}", printed(&module));
}
