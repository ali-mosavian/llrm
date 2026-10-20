//! Each expected text is what the pinned `opt
//! -passes='simplifycfg<no-speculate-blocks;switch-range-to-icmp>'` prints.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{Declared, Outer};

use crate::simplifycfg::{Options, simplified_with};
use crate::testing::{parsed, printed, results};
use crate::twoentryphi::Speculation;

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
    let options = Options { speculate_blocks: false, ..Options::default() };
    let (context, function) = module.function_mut("f").expect("@f");
    assert_eq!(simplified_with(&options, context, &spec, &mut declared, function), text != expected, "{text}");
    assert_eq!(printed(&module), expected);
    assert_eq!(results(&module, inputs), before);
}

/// Cases 1, 2, 3 to one block stayed a three-way switch, not `x - 1 u< 3`.
#[test]
fn test_cases_in_a_run_are_one_range_compare() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %b2 [
    i16 1, label %b1
    i16 2, label %b1
    i16 3, label %b1
  ]

b1:
  store i16 1, ptr @g
  ret i16 1

b2:
  store i16 0, ptr @g
  ret i16 0
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  %x.off = add i16 %x, -1
  %switch = icmp ult i16 %x.off, 3
  br i1 %switch, label %b1, label %b2

common.ret:
  %common.op = phi i16 [ 1, %b1 ], [ 0, %b2 ]
  ret i16 %common.op

b1:
  store i16 1, ptr @g
  br label %common.ret

b2:
  store i16 0, ptr @g
  br label %common.ret
}
",
        &[&[0], &[1], &[2], &[3], &[4]],
    );
}

/// A run starting at 0 needs no `add`.
#[test]
fn test_a_run_from_zero_has_no_offset() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %b2 [
    i16 0, label %b1
    i16 1, label %b1
  ]

b1:
  store i16 1, ptr @g
  ret i16 1

b2:
  store i16 0, ptr @g
  ret i16 0
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  %switch = icmp ult i16 %x, 2
  br i1 %switch, label %b1, label %b2

common.ret:
  %common.op = phi i16 [ 1, %b1 ], [ 0, %b2 ]
  ret i16 %common.op

b1:
  store i16 1, ptr @g
  br label %common.ret

b2:
  store i16 0, ptr @g
  br label %common.ret
}
",
        &[&[0], &[1], &[2]],
    );
}

/// Three edges into a phi stayed three inputs after the switch became a
/// branch with one.
#[test]
fn test_the_phi_of_the_run_keeps_one_input() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %b2 [
    i16 1, label %b1
    i16 2, label %b1
    i16 3, label %b1
  ]

b1:
  %r = phi i16 [ 5, %b0 ], [ 5, %b0 ], [ 5, %b0 ]
  ret i16 %r

b2:
  store i16 0, ptr @g
  ret i16 0
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  %x.off = add i16 %x, -1
  %switch = icmp ult i16 %x.off, 3
  br i1 %switch, label %common.ret, label %b2

common.ret:
  %common.op = phi i16 [ 0, %b2 ], [ 5, %b0 ]
  ret i16 %common.op

b2:
  store i16 0, ptr @g
  br label %common.ret
}
",
        &[&[0], &[1], &[2], &[3], &[4]],
    );
}

/// An unreachable default and one case per arm: one `icmp eq`.
#[test]
fn test_two_single_cases_are_an_equality() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %u [
    i16 4, label %b1
    i16 9, label %b2
  ]

b1:
  store i16 1, ptr @g
  ret i16 1

b2:
  store i16 2, ptr @g
  ret i16 2

u:
  unreachable
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  %0 = icmp eq i16 %x, 4
  br i1 %0, label %b1, label %b2

common.ret:
  %common.op = phi i16 [ 1, %b1 ], [ 2, %b2 ]
  ret i16 %common.op

b1:
  store i16 1, ptr @g
  br label %common.ret

b2:
  store i16 2, ptr @g
  br label %common.ret
}
",
        &[&[4], &[9]],
    );
}

/// Cases 0, 1, 6, 7 of a 3-bit value are a run that wraps: the other arm's
/// 2..=5 is the one compared.
#[test]
fn test_a_wrapping_run_is_compared_as_the_other_arms_run() {
    check(
        "@g = global i16 0

define i16 @f(i16 %a) {
b0:
  %x = and i16 %a, 7
  switch i16 %x, label %u [
    i16 0, label %b1
    i16 1, label %b1
    i16 6, label %b1
    i16 7, label %b1
    i16 2, label %b2
    i16 3, label %b2
    i16 4, label %b2
    i16 5, label %b2
  ]

b1:
  store i16 1, ptr @g
  ret i16 1

b2:
  store i16 2, ptr @g
  ret i16 2

u:
  unreachable
}
",
        "@g = global i16 0

define i16 @f(i16 %a) {
b0:
  %x = and i16 %a, 7
  %x.off = add i16 %x, -2
  %switch = icmp ult i16 %x.off, 4
  br i1 %switch, label %b2, label %b1

common.ret:
  %common.op = phi i16 [ 1, %b1 ], [ 2, %b2 ]
  ret i16 %common.op

b1:
  store i16 1, ptr @g
  br label %common.ret

b2:
  store i16 2, ptr @g
  br label %common.ret
}
",
        &[&[0], &[1], &[2], &[5], &[6], &[7], &[8]],
    );
}
