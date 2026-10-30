use super::branches_optimized;
use crate::testing::{parsed, printed};

struct Zero(bool);

impl llrm_mir::lowering::Lowering for Zero {
    fn prefer_zero_compare_branch(&self) -> bool {
        self.0
    }
}

fn after(
    text: &str,
    prefer: bool,
) -> (bool, String) {
    let mut module = parsed(text);
    let (context, function) = {
        let llrm_mir::module::Module { context, globals, .. } = &mut module;
        let llrm_mir::module::GlobalKind::Function(function) = &mut globals[0].kind else { panic!("a function") };
        (&*context, function)
    };
    let changed = branches_optimized(context, function, Some(&Zero(prefer)));
    (changed, printed(&module))
}

const BELOW: &str = "define i16 @f(i16 %x) {
b0:
  %c = icmp ult i16 %x, 8
  br i1 %c, label %b1, label %b2

b1:
  %t = lshr i16 %x, 3
  ret i16 %t

b2:
  ret i16 0
}
";

/// `icmp ult %x, 8` then `lshr %x, 3` in a successor: the pin compares the
/// shift with zero (CodeGenPrepare.cpp `optimizeBranch`), which the shift's
/// flags then serve. The shift moves up beside the branch.
#[test]
fn test_an_unsigned_compare_with_a_power_of_two_becomes_a_compare_of_the_shift_with_zero() {
    let (changed, text) = after(BELOW, true);
    assert!(changed, "{text}");
    assert!(text.contains("%t = lshr i16 %x, 3\n  %"), "the shift is before the branch: {text}");
    assert!(text.contains("icmp eq i16 %t, 0"), "{text}");
    assert!(!text.contains("icmp ult"), "{text}");
}

#[test]
fn test_a_target_that_does_not_prefer_a_zero_compare_keeps_the_branch() {
    let (changed, text) = after(BELOW, false);
    assert!(!changed, "{text}");
    assert!(text.contains("icmp ult i16 %x, 8"), "{text}");
}

#[test]
fn test_an_equality_with_an_add_of_the_negated_constant_compares_the_add_with_zero() {
    let (changed, text) = after(
        "define i16 @f(i16 %x) {
b0:
  %c = icmp eq i16 %x, 5
  br i1 %c, label %b1, label %b2

b1:
  %t = add i16 %x, -5
  ret i16 %t

b2:
  ret i16 0
}
",
        true,
    );
    assert!(changed, "{text}");
    assert!(text.contains("icmp eq i16 %t, 0"), "{text}");
}

#[test]
fn test_a_compare_with_a_second_use_is_left() {
    let (changed, text) = after(
        "define i16 @f(i16 %x) {
b0:
  %c = icmp ult i16 %x, 8
  %z = zext i1 %c to i16
  br i1 %c, label %b1, label %b2

b1:
  %t = lshr i16 %x, 3
  ret i16 %t

b2:
  ret i16 %z
}
",
        true,
    );
    assert!(!changed, "{text}");
}
