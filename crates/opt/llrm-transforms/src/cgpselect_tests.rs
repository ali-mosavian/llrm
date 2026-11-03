use llrm_mir::datalayout::DataLayout;

use super::casts_branched;
use crate::testing::{parsed, printed};

/// The clamp of x_ldcmp: `(long)(x < -3 ? -3 : x > 4 ? 4 : x)`.
const CLAMP: &str = "define i32 @f(double %x) {
b0:
  %lo = fcmp olt double %x, -3.0
  %hi = fcmp ogt double %x, 4.0
  %up = select i1 %hi, double 4.0, double %x
  %s = select i1 %lo, double -3.0, double %up
  %r = fptosi double %s to i32
  ret i32 %r
}
";

fn after(
    text: &str,
    conversion: i64,
    branch: i64,
) -> (bool, String) {
    let mut module = parsed(text);
    let layout = DataLayout::default();
    let llrm_mir::module::Module { context, globals, .. } = &mut module;
    let llrm_mir::module::GlobalKind::Function(function) = &mut globals[0].kind else { panic!("a function") };
    let changed = casts_branched(context, &layout, function, conversion, branch);
    (changed, printed(&module))
}

/// Both constants are converted here and the conversion of `%x` is made once,
/// where neither constant is taken (x_ldcmp -O2: 2020 `fistp` against gcc's
/// 1180).
#[test]
fn test_a_dear_conversion_of_a_clamp_is_made_on_the_path_that_takes_neither_constant() {
    let (changed, text) = after(CLAMP, 43, 3);
    assert!(changed, "{text}");
    assert_eq!(text.matches("fptosi").count(), 1, "{text}");
    assert!(text.contains("phi i32 [ -3, %b0 ]") || text.contains("phi i32 [-3, %b0]"), "{text}");
    assert!(text.contains("phi i32 [ 4,") || text.contains("[ 4, "), "{text}");
}

/// Control: where one instruction converts, the select stays.
#[test]
fn test_a_cheap_conversion_keeps_the_select() {
    let (changed, text) = after(CLAMP, 1, 3);
    assert!(!changed, "{text}");
    assert!(text.contains("select i1 %lo"), "{text}");
}

/// Control: a select of two values has no constant to convert at compile time.
#[test]
fn test_a_select_without_a_constant_arm_keeps_its_conversion_after_it() {
    let (changed, text) = after(
        "define i32 @f(double %x, double %y, i1 %c) {
b0:
  %s = select i1 %c, double %x, double %y
  %r = fptosi double %s to i32
  ret i32 %r
}
",
        43,
        3,
    );
    assert!(!changed, "{text}");
}

/// Control: a select the conversion does not own alone keeps its other reader.
#[test]
fn test_a_select_with_a_second_reader_is_left() {
    let (changed, text) = after(
        "define i32 @f(double %x, i1 %c, ptr %p) {
b0:
  %s = select i1 %c, double 4.0, double %x
  store double %s, ptr %p
  %r = fptosi double %s to i32
  ret i32 %r
}
",
        43,
        3,
    );
    assert!(!changed, "{text}");
}
