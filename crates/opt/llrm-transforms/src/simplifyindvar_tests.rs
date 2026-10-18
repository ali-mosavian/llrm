//! Comparisons with an induction variable, eliminated by ScalarEvolution.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::Outer;

use crate::testing::{parsed, printed, results};

fn simplified(text: &str) -> (bool, String) {
    let mut module = parsed(&format!("{DOS}{text}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = super::simplified(context, &layout, function, &outer);
    (changed, printed(&module))
}

/// `n` is below 100, so `i < n` makes `i < 200` true on every trip: the
/// compare is `true` wherever it is read.
const BELOW: &str = "define i16 @f(i16 range(i16 0, 100) %n) {
entry:
  br label %head
head:
  %i = phi i16 [ 0, %entry ], [ %j, %next ]
  %more = icmp ult i16 %i, %n
  br i1 %more, label %body, label %done
body:
  %big = icmp ult i16 %i, 200
  br i1 %big, label %next, label %crash
next:
  %j = add nuw i16 %i, 1
  br label %head
crash:
  ret i16 99
done:
  ret i16 0
}
";

#[test]
fn test_a_compare_the_trip_count_decides_is_a_constant() {
    let (changed, after) = simplified(BELOW);
    assert!(changed, "{after}");
    assert!(after.contains("br i1 true, label %next"), "{after}");
    let before = parsed(&format!("{DOS}{BELOW}"));
    let mut module = parsed(&format!("{DOS}{BELOW}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    super::simplified(context, &layout, function, &outer);
    assert_eq!(results(&module, &[&[0], &[5], &[99]]), results(&before, &[&[0], &[5], &[99]]));
}

/// A compare the trip count does not decide is left: `i < 50` for `n` below
/// 100 may fail.
#[test]
fn test_a_compare_nothing_decides_is_left() {
    let (changed, after) = simplified(&BELOW.replace("%i, 200", "%i, 50"));
    assert!(!changed, "{after}");
    assert!(after.contains("br i1 %big"), "{after}");
}
