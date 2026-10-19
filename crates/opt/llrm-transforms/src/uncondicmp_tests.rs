//! Each body is MIR text run by llrm-mir's interpreter before and after.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::Outer;

use crate::simplifycfg::{Options, simplified_with};
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
    let options = Options { speculate_blocks: false, ..Default::default() };
    let (context, function) = module.function_mut("f").expect("@f");
    assert_eq!(simplified_with(&options, context, &spec, function), text != expected, "{text}");
    assert_eq!(printed(&module), expected);
    assert_eq!(results(&module, inputs), before);
}

/// The default's compare and select become a case of the switch of their own:
/// the select no longer runs, its two values come to the join by two edges.
#[test]
fn test_a_compare_and_select_in_the_default_become_a_case() {
    check(
        "define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 0, label %e
    i16 1, label %c
  ]

c:
  br label %e

d:
  %t = icmp eq i16 %x, 2
  %v = select i1 %t, i16 7, i16 9
  br label %e

e:
  %r = phi i16 [ 5, %b0 ], [ 6, %c ], [ %v, %d ]
  ret i16 %r
}
",
        "define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 0, label %e
    i16 1, label %c
    i16 2, label %switch.edge
  ]

switch.edge:
  br label %e

c:
  br label %e

d:
  br label %e

e:
  %r = phi i16 [ 5, %b0 ], [ 6, %c ], [ 9, %d ], [ 7, %switch.edge ]
  ret i16 %r
}
",
        &[&[0], &[1], &[2], &[3]],
    );
}

/// A compare alone: the phi of `i1` gets the compare's two values.
#[test]
fn test_a_compare_alone_in_the_default_becomes_a_case() {
    check(
        "define i1 @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 0, label %e
    i16 1, label %c
  ]

c:
  br label %e

d:
  %t = icmp ne i16 %x, 2
  br label %e

e:
  %r = phi i1 [ true, %b0 ], [ false, %c ], [ %t, %d ]
  ret i1 %r
}
",
        "define i1 @f(i16 %x) {
b0:
  switch i16 %x, label %e [
    i16 1, label %c
    i16 2, label %switch.edge
  ]

switch.edge:
  br label %e

c:
  br label %e

e:
  %r = phi i1 [ true, %b0 ], [ false, %c ], [ false, %switch.edge ]
  ret i1 %r
}
",
        &[&[0], &[1], &[2], &[3]],
    );
}

/// A compare with a value another case has is false in the default.
#[test]
fn test_a_compare_with_a_case_value_in_the_default_is_known() {
    check(
        "define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 0, label %e
    i16 1, label %c
  ]

c:
  br label %e

d:
  %t = icmp eq i16 %x, 1
  %v = select i1 %t, i16 7, i16 9
  br label %e

e:
  %r = phi i16 [ 5, %b0 ], [ 6, %c ], [ %v, %d ]
  ret i16 %r
}
",
        "define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 0, label %e
    i16 1, label %c
  ]

c:
  br label %e

d:
  %v = select i1 false, i16 7, i16 9
  br label %e

e:
  %r = phi i16 [ 5, %b0 ], [ 6, %c ], [ %v, %d ]
  ret i16 %r
}
",
        &[&[0], &[1], &[2], &[3]],
    );
}

/// Reached by a case, the value is the case's.
#[test]
fn test_a_compare_in_a_case_block_is_known() {
    check(
        "define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %e [
    i16 3, label %d
    i16 4, label %f
  ]

f:
  br label %e

d:
  %t = icmp eq i16 %x, 3
  %v = select i1 %t, i16 7, i16 9
  br label %e

e:
  %r = phi i16 [ 5, %b0 ], [ %v, %d ], [ 8, %f ]
  ret i16 %r
}
",
        "define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %e [
    i16 3, label %d
    i16 4, label %f
  ]

f:
  br label %e

d:
  %v = select i1 true, i16 7, i16 9
  br label %e

e:
  %r = phi i16 [ 5, %b0 ], [ %v, %d ], [ 8, %f ]
  ret i16 %r
}
",
        &[&[0], &[3], &[4], &[5]],
    );
}
