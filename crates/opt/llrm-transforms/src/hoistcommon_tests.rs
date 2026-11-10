//! Each body is MIR text run by llrm-mir's interpreter before and after.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{Declared, Outer};
use llrm_x86::tti::X86;

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

/// `text`'s `@f` after SimplifyCFG with hoisting on, priced by `tti`.
fn hoisted_with(
    tti: &dyn llrm_mir::tti::Tti,
    text: &str,
) -> String {
    let mut module = parsed(text);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    simplified_with(&branching(true), context, &spec, &mut declared, function);
    printed(&module)
}

/// The arms read the same word and differ in what they do with it.
fn arms_reading_a_word(
    left: &str,
    right: &str,
) -> String {
    format!(
        "define i32 @f(i32 %x, ptr %p, i1 %c) {{
b0:
  br i1 %c, label %b1, label %b2

b1:
  %l = load i32, ptr %p
  %a = {left}
  br label %b3

b2:
  %r = load i32, ptr %p
  %b = {right}
  br label %b3

b3:
  %v = phi i32 [ %a, %b1 ], [ %b, %b2 ]
  ret i32 %v
}}
"
    )
}

/// x_cordic -O1 (m32): `atn[k]` was read once before the branch and taken by
/// `sub` and `add` as a register, so the inner loop needed a 7th register and
/// the outer loop spilled twice (+397 clk, +8 B). On x86 each arm keeps the
/// load its own `sub`/`add` takes as a memory operand.
#[test]
fn test_a_load_each_arm_folds_stays_in_its_arm_on_x86() {
    let text = arms_reading_a_word("sub i32 %x, %l", "add i32 %x, %r");
    let x86 = X86::of_cpu("i486").expect("a cpu");
    let on_x86 = hoisted_with(&x86, &text);
    assert_eq!(on_x86.matches("load i32").count(), 2, "{on_x86}");
    let pin = hoisted_with(&llrm_mir::tti::Basic, &text);
    assert_eq!(pin.matches("load i32").count(), 1, "the base answer hoists it: {pin}");
}

/// A shift count is not a memory operand: nothing is lost by sharing the load.
#[test]
fn test_a_load_that_cannot_fold_is_still_hoisted_on_x86() {
    let text = arms_reading_a_word("shl i32 %x, %l", "lshr i32 %x, %r");
    let on_x86 = hoisted_with(&X86::of_cpu("i486").expect("a cpu"), &text);
    assert_eq!(on_x86.matches("load i32").count(), 1, "{on_x86}");
}

/// `sub` takes memory as its second operand only.
#[test]
fn test_a_load_that_is_the_first_operand_of_a_sub_is_still_hoisted_on_x86() {
    let text = arms_reading_a_word("sub i32 %l, %x", "sub i32 %r, %x");
    let on_x86 = hoisted_with(&X86::of_cpu("i486").expect("a cpu"), &text);
    assert_eq!(on_x86.matches("load i32").count(), 1, "{on_x86}");
}

/// Against an immediate the memory form (`cmp [m],imm`) is longer than the
/// shared load plus `test`: folding it grew sfm16_dbg -Os by 1 B at m16/m32 and
/// 2 B at m64 (identity sweep, 2026-11-10).
#[test]
fn test_a_load_compared_with_a_constant_is_still_hoisted_on_x86() {
    let text = format!(
        "define i1 @f(ptr %p, i1 %c) {{
b0:
  br i1 %c, label %b1, label %b2

b1:
  %l = load i32, ptr %p
  %a = icmp eq i32 %l, 0
  br label %b3

b2:
  %r = load i32, ptr %p
  %b = icmp slt i32 %r, 0
  br label %b3

b3:
  %v = phi i1 [ %a, %b1 ], [ %b, %b2 ]
  ret i1 %v
}}
"
    );
    let on_x86 = hoisted_with(&X86::of_cpu("i486").expect("a cpu"), &text);
    assert_eq!(on_x86.matches("load i32").count(), 1, "{on_x86}");
}
