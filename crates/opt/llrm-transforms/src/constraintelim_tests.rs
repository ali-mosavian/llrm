//! `constraint-elimination` on MIR text: what the dominating conditions
//! decide becomes a constant, and nothing else changes.

use llrm_mir::datalayout::DataLayout;

use crate::constraintelim::eliminate_constraints;
use crate::testing::{parsed, printed};

/// The text of the module after the pass has run on @f.
fn eliminated(text: &str) -> String {
    let mut module = parsed(text);
    let layout = DataLayout::default();
    let declarations = module.declarations();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    eliminate_constraints(context, &layout, &declarations, &metadata, function);
    printed(&module)
}

/// `x < 5` makes the inner `x < 10` true.
#[test]
fn a_dominating_compare_decides_an_implied_one() {
    let text = eliminated(
        "define i1 @f(i32 %x) {
b0:
  %c = icmp ult i32 %x, 5
  br i1 %c, label %b1, label %b2
b1:
  %d = icmp ult i32 %x, 10
  ret i1 %d
b2:
  ret i1 false
}
",
    );
    assert!(text.contains("ret i1 true"), "{text}");
}

/// A fact that does not decide the compare leaves it alone.
#[test]
fn an_undecided_compare_is_left_alone() {
    let text = eliminated(
        "define i1 @f(i32 %x) {
b0:
  %c = icmp ult i32 %x, 10
  br i1 %c, label %b1, label %b2
b1:
  %d = icmp ult i32 %x, 5
  ret i1 %d
b2:
  ret i1 false
}
",
    );
    assert!(text.contains("ret i1 %d"), "{text}");
}

/// The fact of one arm does not hold in the other.
#[test]
fn a_fact_does_not_leave_its_arm() {
    let text = eliminated(
        "define i1 @f(i32 %x) {
b0:
  %c = icmp ult i32 %x, 5
  br i1 %c, label %b1, label %b2
b1:
  ret i1 false
b2:
  %d = icmp ult i32 %x, 10
  ret i1 %d
}
",
    );
    assert!(text.contains("ret i1 %d"), "{text}");
}
