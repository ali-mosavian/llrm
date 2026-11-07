//! `combine_select_tail`: selects of selects that one select with the inner
//! condition outermost or a xor condition replaces.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Select { c.visit_select(inst) } else { Visit::None }
}

fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// `select c2, (select c1, a, b), (select c1, b, a)` stayed three selects;
/// pinned `opt -passes=instcombine` gives `select (xor c1, c2), b, a`.
#[test]
fn a_select_of_a_symmetric_pair_is_a_select_on_the_xor() {
    let after = combined(
        "define i8 @f(i1 %c1, i1 %c2, i8 %a, i8 %b) {
b0:
  %i1 = select i1 %c1, i8 %a, i8 %b
  %i2 = select i1 %c1, i8 %b, i8 %a
  %r = select i1 %c2, i8 %i1, i8 %i2
  ret i8 %r
}
",
    );
    assert!(after.contains("xor i1 %c1, %c2"), "{after}");
    assert_eq!(after.matches("select").count(), 1, "{after}");
    assert!(after.contains(", i8 %b, i8 %a"), "{after}");
}

/// The inner pair is not symmetric (same arms): left alone.
#[test]
fn a_select_of_a_pair_with_the_same_arms_is_not_symmetric() {
    let after = combined(
        "define i8 @f(i1 %c1, i1 %c2, i8 %a, i8 %b) {
b0:
  %i1 = select i1 %c1, i8 %a, i8 %b
  %i2 = select i1 %c1, i8 %a, i8 %b
  %r = select i1 %c2, i8 %i1, i8 %i2
  ret i8 %r
}
",
    );
    assert!(!after.contains("xor"), "{after}");
}

/// `select (a && b), x, (select a, y, z)` needed three selects.
#[test]
fn a_select_on_an_and_of_the_inner_condition_has_the_inner_condition_outermost() {
    let after = combined(
        "define i8 @f(i1 %a, i1 %b, i8 %x, i8 %y, i8 %z) {
b0:
  %c = select i1 %a, i1 %b, i1 false
  %i = select i1 %a, i8 %y, i8 %z
  %r = select i1 %c, i8 %x, i8 %i
  ret i8 %r
}
",
    );
    assert!(after.contains("select i1 %b, i8 %x, i8 %y"), "{after}");
    assert!(after.contains("select i1 %a, i8 %") && after.contains(", i8 %z"), "{after}");
    assert!(!after.contains("select i1 %a, i1 %b"), "{after}");
}

/// `select (a || b), (select a, y, z), x`.
#[test]
fn a_select_on_an_or_of_the_inner_condition_has_the_inner_condition_outermost() {
    let after = combined(
        "define i8 @f(i1 %a, i1 %b, i8 %x, i8 %y, i8 %z) {
b0:
  %c = select i1 %a, i1 true, i1 %b
  %i = select i1 %a, i8 %y, i8 %z
  %r = select i1 %c, i8 %i, i8 %x
  ret i8 %r
}
",
    );
    assert!(after.contains("select i1 %b, i8 %z, i8 %x"), "{after}");
    assert!(after.contains("select i1 %a, i8 %y, i8 %"), "{after}");
}

/// The logical op's operand is the inverse of the inner condition.
#[test]
fn a_select_on_an_and_of_the_negated_inner_condition_swaps_the_inner_arms() {
    let after = combined(
        "define i8 @f(i1 %a, i1 %b, i8 %x, i8 %y, i8 %z) {
b0:
  %na = xor i1 %a, true
  %c = select i1 %na, i1 %b, i1 false
  %i = select i1 %a, i8 %y, i8 %z
  %r = select i1 %c, i8 %x, i8 %i
  ret i8 %r
}
",
    );
    assert!(after.contains("select i1 %b, i8 %x, i8 %z"), "{after}");
    assert!(after.contains("select i1 %a, i8 %y, i8 %"), "{after}");
}
