//! The first two expected texts are what the pinned
//! `opt -passes='simplifycfg<no-speculate-blocks>'` prints. The third is
//! derived by hand: `opt` also removes the dead case in
//! `eliminateDeadSwitchCases`, which is not translated yet.

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
    let options = Options { speculate_blocks: false, convert_switch_range_to_icmp: false, ..Options::default() };
    let (context, function) = module.function_mut("f").expect("@f");
    assert_eq!(simplified_with(&options, context, &spec, &mut declared, function), text != expected, "{text}");
    assert_eq!(printed(&module), expected);
    assert_eq!(results(&module, inputs), before);
}

/// Two arms that were each a jump to the join with the same phi input stayed
/// two blocks.
#[test]
fn test_identical_switch_arms_are_one() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %m [
    i16 1, label %e1
    i16 2, label %e2
    i16 3, label %s
  ]

e1:
  br label %m

e2:
  br label %m

s:
  store i16 7, ptr @g
  br label %m

m:
  %r = phi i16 [ 0, %b0 ], [ 5, %e1 ], [ 5, %e2 ], [ 9, %s ]
  ret i16 %r
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %m [
    i16 1, label %e1
    i16 2, label %e1
    i16 3, label %s
  ]

e1:
  br label %m

s:
  store i16 7, ptr @g
  br label %m

m:
  %r = phi i16 [ 0, %b0 ], [ 5, %e1 ], [ 9, %s ]
  ret i16 %r
}
",
        &[&[0], &[1], &[2], &[3]],
    );
}

/// `umin(%a, 3)` as the switched value kept the case 5 it can never take and
/// the unreachable default.
#[test]
fn test_a_switch_on_umin_drops_cases_above_it_and_takes_the_bound_as_default() {
    check(
        "@g = global i16 0

declare i16 @llvm.umin.i16(i16, i16)

define i16 @f(i16 %a) {
b0:
  %c = call i16 @llvm.umin.i16(i16 %a, i16 3)
  switch i16 %c, label %u [
    i16 0, label %b1
    i16 1, label %b2
    i16 3, label %b3
    i16 5, label %b4
  ]

b1:
  store i16 1, ptr @g
  ret i16 1

b2:
  store i16 2, ptr @g
  ret i16 2

b3:
  store i16 3, ptr @g
  ret i16 3

b4:
  store i16 4, ptr @g
  ret i16 4

u:
  unreachable
}
",
        "@g = global i16 0

declare i16 @llvm.umin.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

define i16 @f(i16 %a) {
b0:
  %c = call i16 @llvm.umin.i16(i16 %a, i16 3)
  switch i16 %a, label %b3 [
    i16 0, label %b1
    i16 1, label %b2
  ]

common.ret:
  %common.op = phi i16 [ 1, %b1 ], [ 2, %b2 ], [ 3, %b3 ]
  ret i16 %common.op

b1:
  store i16 1, ptr @g
  br label %common.ret

b2:
  store i16 2, ptr @g
  br label %common.ret

b3:
  store i16 3, ptr @g
  br label %common.ret
}
",
        &[&[0], &[1], &[3], &[9]],
    );
}

/// A value fixed to 0 where the default runs stayed a reachable default.
#[test]
fn test_a_known_constant_value_makes_the_default_unreachable() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  %c = and i16 %x, 0
  switch i16 %c, label %d [
    i16 1, label %b1
    i16 7, label %b2
  ]

b1:
  store i16 1, ptr @g
  ret i16 1

b2:
  store i16 2, ptr @g
  ret i16 2

d:
  store i16 3, ptr @g
  ret i16 3
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  %c = and i16 %x, 0
  switch i16 %c, label %b0.unreachabledefault [
    i16 1, label %b1
    i16 7, label %b2
    i16 0, label %d
  ]

common.ret:
  %common.op = phi i16 [ 1, %b1 ], [ 2, %b2 ], [ 3, %d ]
  ret i16 %common.op

b1:
  store i16 1, ptr @g
  br label %common.ret

b2:
  store i16 2, ptr @g
  br label %common.ret

b0.unreachabledefault:
  unreachable

d:
  store i16 3, ptr @g
  br label %common.ret
}
",
        &[&[0], &[5]],
    );
}
