//! Each expected text is what the pinned `opt
//! -passes='simplifycfg<no-speculate-blocks;forward-switch-cond>'` prints.

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
    let options = Options {
        speculate_blocks: false,
        convert_switch_range_to_icmp: false,
        forward_switch_cond_to_phi: true,
        ..Options::default()
    };
    let (context, function) = module.function_mut("f").expect("@f");
    assert_eq!(simplified_with(&options, context, &spec, &mut declared, function), text != expected, "{text}");
    assert_eq!(printed(&module), expected);
    assert_eq!(results(&module, inputs), before);
}

/// Two cases jumping through empty blocks to a phi of their constants kept
/// both blocks and both constants instead of one input, `%x`, for each.
#[test]
fn test_phi_inputs_of_cases_through_empty_blocks_are_the_switched_value() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 1, label %e1
    i16 2, label %e2
  ]

e1:
  br label %m

e2:
  br label %m

d:
  store i16 7, ptr @g
  br label %m

m:
  %r = phi i16 [ 1, %e1 ], [ 2, %e2 ], [ 0, %d ]
  ret i16 %r
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 1, label %m
    i16 2, label %m
  ]

d:
  store i16 7, ptr @g
  br label %m

m:
  %r = phi i16 [ 0, %d ], [ %x, %b0 ], [ %x, %b0 ]
  ret i16 %r
}
",
        &[&[0], &[1], &[2], &[3]],
    );
}

/// A phi input on the edge of a case, with the case's constant, stayed the
/// constant instead of the switched value.
#[test]
fn test_a_phi_input_on_a_case_edge_is_the_switched_value() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 1, label %m
    i16 2, label %n
  ]

d:
  store i16 7, ptr @g
  br label %m

n:
  store i16 8, ptr @g
  ret i16 9

m:
  %r = phi i16 [ 1, %b0 ], [ 0, %d ]
  ret i16 %r
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 1, label %common.ret
    i16 2, label %n
  ]

d:
  store i16 7, ptr @g
  br label %common.ret

common.ret:
  %common.op = phi i16 [ 9, %n ], [ %x, %b0 ], [ 0, %d ]
  ret i16 %common.op

n:
  store i16 8, ptr @g
  br label %common.ret
}
",
        &[&[0], &[1], &[2], &[3]],
    );
}

/// Two cases to one block with the same constant: the input is not the
/// switched value of either, and stayed the constant.
#[test]
fn test_two_edges_with_one_constant_keep_it() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 5, label %m
    i16 1, label %m
  ]

d:
  store i16 7, ptr @g
  br label %m

m:
  %r = phi i16 [ 5, %b0 ], [ 5, %b0 ], [ 0, %d ]
  ret i16 %r
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 5, label %m
    i16 1, label %m
  ]

d:
  store i16 7, ptr @g
  br label %m

m:
  %r = phi i16 [ 5, %b0 ], [ 5, %b0 ], [ 0, %d ]
  ret i16 %r
}
",
        &[&[0], &[1], &[5]],
    );
}
