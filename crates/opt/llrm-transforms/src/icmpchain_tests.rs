//! The expected text is the whole pass's result: the switch folds that landed
//! after these were written (select, range) run on what the step under test
//! leaves, as in the pin.

//! Each body is MIR text run by llrm-mir's interpreter before and after.

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

/// `x == 1 || x == 5 || x == 9` stayed a chain of three compares and two ors.
#[test]
fn test_a_chain_of_equalities_is_a_switch() {
    check(
        "define i16 @f(i16 %x) {
b0:
  %a = icmp eq i16 %x, 1
  %b = icmp eq i16 %x, 5
  %c = icmp eq i16 %x, 9
  %ab = or i1 %a, %b
  %abc = or i1 %ab, %c
  br i1 %abc, label %b1, label %b2

b1:
  ret i16 1

b2:
  ret i16 0
}
",
        "define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %b2 [
    i16 9, label %common.ret
    i16 5, label %common.ret
    i16 1, label %common.ret
  ]

common.ret:
  %common.op = phi i16 [ 0, %b2 ], [ 1, %b0 ], [ 1, %b0 ], [ 1, %b0 ]
  ret i16 %common.op

b2:
  br label %common.ret
}
",
        &[&[0], &[1], &[5], &[9], &[2]],
    );
}

/// `x != 1 && x != 5 && x != 9` is the same switch with its arms swapped.
#[test]
fn test_a_chain_of_inequalities_is_a_switch_with_its_arms_swapped() {
    check(
        "define i16 @f(i16 %x) {
b0:
  %a = icmp ne i16 %x, 1
  %b = icmp ne i16 %x, 5
  %c = icmp ne i16 %x, 9
  %ab = and i1 %a, %b
  %abc = and i1 %ab, %c
  br i1 %abc, label %b1, label %b2

b1:
  ret i16 0

b2:
  ret i16 1
}
",
        "define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %common.ret [
    i16 9, label %b2
    i16 5, label %b2
    i16 1, label %b2
  ]

common.ret:
  %common.op = phi i16 [ 1, %b2 ], [ 0, %b0 ]
  ret i16 %common.op

b2:
  br label %common.ret
}
",
        &[&[0], &[1], &[5], &[9], &[2]],
    );
}

/// `x == 1 || x == 2 || x == 3` is one range check, not a switch.
#[test]
fn test_a_run_of_equalities_is_a_range_check() {
    check(
        "define i16 @f(i16 %x) {
b0:
  %a = icmp eq i16 %x, 1
  %b = icmp eq i16 %x, 2
  %c = icmp eq i16 %x, 3
  %ab = or i1 %a, %b
  %abc = or i1 %ab, %c
  br i1 %abc, label %b1, label %b2

b1:
  ret i16 1

b2:
  ret i16 0
}
",
        "define i16 @f(i16 %x) {
b0:
  %0 = add i16 %x, -1
  %1 = icmp ult i16 %0, 3
  %2 = select i1 %1, i16 1, i16 0
  ret i16 %2
}
",
        &[&[0], &[1], &[2], &[3], &[4]],
    );
}

/// One clause that is not a compare of `%x` is tested first, frozen.
#[test]
fn test_one_other_clause_is_tested_before_the_switch() {
    check(
        "define i16 @f(i16 %x, i1 %y) {
b0:
  %a = icmp eq i16 %x, 1
  %b = icmp eq i16 %x, 5
  %ab = or i1 %a, %b
  %aby = or i1 %ab, %y
  br i1 %aby, label %b1, label %b2

b1:
  ret i16 1

b2:
  ret i16 0
}
",
        "define i16 @f(i16 %x, i1 %y) {
b0:
  %0 = freeze i1 %y
  %1 = sub i16 %x, 1
  %2 = and i16 %1, -5
  %3 = icmp eq i16 %2, 0
  %4 = select i1 %3, i16 1, i16 0
  %5 = select i1 %0, i16 1, i16 %4
  ret i16 %5
}
",
        &[&[0, 0], &[1, 0], &[5, 0], &[2, 1], &[2, 0]],
    );
}

/// A phi in the target gets one input per case.
#[test]
fn test_the_targets_phi_has_an_input_for_each_case() {
    check(
        "define i16 @f(i16 %x) {
b0:
  %a = icmp eq i16 %x, 1
  %b = icmp eq i16 %x, 5
  %ab = or i1 %a, %b
  br i1 %ab, label %b1, label %b2

b1:
  %r = phi i16 [ 7, %b0 ], [ 3, %b2 ]
  ret i16 %r

b2:
  br label %b1
}
",
        "define i16 @f(i16 %x) {
b0:
  %0 = sub i16 %x, 1
  %1 = and i16 %0, -5
  %2 = icmp eq i16 %1, 0
  %3 = select i1 %2, i16 7, i16 3
  ret i16 %3
}
",
        &[&[0], &[1], &[5], &[2]],
    );
}

/// Every conditional branch built a `Query` and ran the gatherer, 1,741 times
/// in d_faces -O2, 773 Ir a call against the pin's 124: a branch on a lone
/// compare or on an argument gathers nothing. Only a logical or/and root does.
#[test]
fn test_only_a_logical_or_and_root_is_gathered() {
    let gathered = |text: &str| {
        let mut module = parsed(text);
        let outer = Outer::of(&module, None);
        let layout = DataLayout::default();
        let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
        let mut declared = Declared::of(&module);
        let (context, function) = module.function_mut("f").expect("@f");
        super::GATHERED.with(|one| one.set(0));
        simplified(context, &spec, &mut declared, function);
        super::GATHERED.with(std::cell::Cell::get)
    };
    let lone = "define i16 @f(i16 %x) {
b0:
  %a = icmp slt i16 %x, 4
  br i1 %a, label %t, label %e

t:
  ret i16 1

e:
  ret i16 2
}
";
    let chain = "define i16 @f(i16 %x) {
b0:
  %a = icmp eq i16 %x, 1
  %b = icmp eq i16 %x, 5
  %c = or i1 %a, %b
  br i1 %c, label %t, label %e

t:
  ret i16 1

e:
  ret i16 2
}
";
    assert_eq!(gathered(lone), 0, "a lone compare is no chain");
    assert!(gathered(chain) >= 1, "an or of compares is gathered");
}
