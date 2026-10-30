//! Each body is MIR text run by llrm-mir's interpreter before and after; the
//! expected text is what the pinned `opt -passes=simplifycfg` prints.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{Declared, Outer};

use crate::simplifycfg::simplified;
use crate::testing::{parsed, printed, results};
use crate::twoentryphi::Speculation;

/// `text` simplified is `expected`, and computes what it did on `inputs`.
fn check(
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
    assert_eq!(simplified(context, &spec, &mut declared, function), text != expected, "{text}");
    assert_eq!(printed(&module), expected);
    assert_eq!(results(&module, inputs), before);
}

/// @f of `cases` to @x, a default @d and the join @end, with `phi` its phi.
fn join(
    prelude: &str,
    cases: &str,
    x_body: &str,
    phi: &str,
) -> String {
    format!(
        "define i32 @f(i32 %a) {{
b0:
{prelude}  switch i32 %{}, label %d [
{cases}  ]

x:
{x_body}  br label %end

y:
  br label %end

d:
  br label %end

end:
  %p = phi i32 {phi}
  ret i32 %p
}}
",
        if prelude.is_empty() { "a" } else { "c" }
    )
}

const INPUTS: &[&[i128]] = &[&[0], &[3], &[4], &[5], &[7], &[10], &[11], &[20], &[21], &[30], &[-1]];

/// Two cases and a default, each its own result, stayed a three-way switch
/// into a phi.
#[test]
fn test_a_switch_of_two_results_and_a_default_is_two_selects() {
    let text = join("", "    i32 10, label %x\n    i32 20, label %y\n", "", "[ 42, %x ], [ 2, %y ], [ 4, %d ]");
    check(
        &text,
        "define i32 @f(i32 %a) {
b0:
  %0 = icmp eq i32 %a, 20
  %1 = select i1 %0, i32 2, i32 4
  %2 = icmp eq i32 %a, 10
  %3 = select i1 %2, i32 42, i32 %1
  ret i32 %3
}
",
        INPUTS,
    );
}

/// Two cases and an unreachable default stayed a switch.
#[test]
fn test_an_unreachable_default_needs_one_select() {
    check(
        "define i32 @f(i32 %a) {
b0:
  switch i32 %a, label %d [
    i32 10, label %x
    i32 20, label %y
  ]

x:
  br label %end

y:
  br label %end

d:
  unreachable

end:
  %p = phi i32 [ 42, %x ], [ 2, %y ]
  ret i32 %p
}
",
        "define i32 @f(i32 %a) {
b0:
  %0 = icmp eq i32 %a, 10
  %1 = select i1 %0, i32 42, i32 2
  ret i32 %1
}
",
        &[&[10], &[20]],
    );
}

/// Cases 0 and 4 to one result stayed a switch; one bit is free, so
/// `a & ~4 == 0` picks the result.
#[test]
fn test_cases_that_differ_in_one_bit_are_a_mask_compare() {
    let text = join("", "    i32 0, label %x\n    i32 4, label %x\n", "", "[ 1, %x ], [ 2, %d ]");
    let text = text.replace("y:\n  br label %end\n\n", "");
    check(
        &text,
        "define i32 @f(i32 %a) {
b0:
  %0 = and i32 %a, -5
  %1 = icmp eq i32 %0, 0
  %2 = select i1 %1, i32 1, i32 2
  ret i32 %2
}
",
        INPUTS,
    );
}

/// Cases 3 and 10 to one result stayed a switch.
#[test]
fn test_two_cases_of_one_result_are_an_or_of_compares() {
    let text = join("", "    i32 3, label %x\n    i32 10, label %x\n", "", "[ 1, %x ], [ 2, %d ]");
    let text = text.replace("y:\n  br label %end\n\n", "");
    check(
        &text,
        "define i32 @f(i32 %a) {
b0:
  %0 = icmp eq i32 %a, 3
  %1 = icmp eq i32 %a, 10
  %2 = or i1 %0, %1
  %3 = select i1 %2, i32 1, i32 2
  ret i32 %3
}
",
        INPUTS,
    );
}

/// Cases 5 and 7 of `a & 7` stayed a switch: the bits both set decide.
#[test]
fn test_bits_every_case_sets_are_an_and_compare() {
    let text = join("  %c = and i32 %a, 7\n", "    i32 5, label %x\n    i32 7, label %x\n", "", "[ 1, %x ], [ 2, %d ]");
    let text = text.replace("y:\n  br label %end\n\n", "");
    check(
        &text,
        "define i32 @f(i32 %a) {
b0:
  %c = and i32 %a, 7
  %0 = and i32 %c, 5
  %1 = icmp eq i32 %0, 5
  %2 = select i1 %1, i32 1, i32 2
  ret i32 %2
}
",
        INPUTS,
    );
}

/// Case blocks that compute from the switch value stayed: each folds to the
/// constant its case gives.
#[test]
fn test_instructions_in_case_blocks_fold_through_the_case_value() {
    check(
        "define i32 @f(i32 %a) {
b0:
  switch i32 %a, label %d [
    i32 10, label %x
    i32 20, label %y
  ]

x:
  %m = add i32 %a, 1
  br label %end

y:
  %n = mul i32 %a, 3
  br label %end

d:
  br label %end

end:
  %p = phi i32 [ %m, %x ], [ %n, %y ], [ 4, %d ]
  ret i32 %p
}
",
        "define i32 @f(i32 %a) {
b0:
  %0 = icmp eq i32 %a, 20
  %1 = select i1 %0, i32 60, i32 4
  %2 = icmp eq i32 %a, 10
  %3 = select i1 %2, i32 11, i32 %1
  ret i32 %3
}
",
        INPUTS,
    );
}

/// Three results stayed a switch: only two are taken.
#[test]
fn test_three_results_are_left_a_switch() {
    let text = "define i32 @f(i32 %a) {
b0:
  switch i32 %a, label %d [
    i32 10, label %x
    i32 20, label %y
    i32 30, label %z
  ]

x:
  br label %end

y:
  br label %end

z:
  br label %end

d:
  br label %end

end:
  %p = phi i32 [ 1, %x ], [ 2, %y ], [ 3, %z ], [ 4, %d ]
  ret i32 %p
}
";
    let mut module = parsed(text);
    let before = results(&module, INPUTS);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    simplified(context, &spec, &mut declared, function);
    assert!(printed(&module).contains("switch i32 %a"), "{}", printed(&module));
    assert_eq!(results(&module, INPUTS), before);
}
