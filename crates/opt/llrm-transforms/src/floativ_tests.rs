//! A counter stepped as a float.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::Outer;

use crate::testing::{parsed, printed};

/// `text`'s @f with its float counters made integer, printed.
fn converted(text: &str) -> (bool, String) {
    let mut module = parsed(&format!("{DOS}{text}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = super::converted(context, &layout, function, &outer);
    (changed, printed(&module))
}

/// `for (double x = 0; x < 100; x += 1) sum += x`.
const COUNTED: &str = "define double @f(double %s) {
entry:
  br label %loop
loop:
  %x = phi double [ 0.0, %entry ], [ %n, %loop ]
  %acc = phi double [ %s, %entry ], [ %a, %loop ]
  %a = fadd double %acc, %x
  %n = fadd double %x, 1.0
  %c = fcmp olt double %n, 100.0
  br i1 %c, label %loop, label %exit
exit:
  ret double %a
}
";

/// The loop was one only an fp compare ended, so ScalarEvolution found no
/// trip count and no later pass could use one.
#[test]
fn a_double_counter_stepped_by_one_is_an_i32_counter() {
    let (changed, after) = converted(COUNTED);
    assert!(changed, "{after}");
    assert!(after.contains("icmp slt i32"), "{after}");
    assert!(after.contains("sitofp i32"), "{after}");
    assert!(!after.contains("fcmp"), "{after}");
}

/// `i += 3` from 0 below 100 lands on 102 in the integers and on 102 in the
/// floats; `!=` would never leave in either.
#[test]
fn an_equality_that_the_step_skips_stays_a_float() {
    let text = COUNTED.replace("olt double %n, 100.0", "one double %n, 100.0").replace("1.0\n  %c", "3.0\n  %c");
    let (changed, after) = converted(&text);
    assert!(!changed, "{after}");
}

/// A bound of 2^24 + 1 is not a `float` any more.
#[test]
fn a_float_bound_past_its_mantissa_stays_a_float() {
    let text = COUNTED.replace("double", "float").replace("100.0", "16777217.0");
    let (changed, after) = converted(&text);
    assert!(!changed, "{after}");
}
