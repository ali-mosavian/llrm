use crate::testing::{parsed, printed};
use crate::widenreturns::widened;

fn run(text: &str) -> String {
    let mut module = parsed(&format!("{}{text}", llrm_analysis::testing::DOS));
    widened(&mut module);
    printed(&module)
}

/// `report(set_bit(x))` paid the zero high word at every call: bit_setclear's
/// four calls of two private functions were 6 B each on the 6502 (151 B against
/// a bar of 147). The function returns the extended value once.
#[test]
fn a_function_whose_callers_all_extend_its_result_returns_the_wider_value() {
    let after = run("define internal i16 @f(i16 %n) {
b0:
  %v = shl i16 1, %n
  ret i16 %v
}
define i32 @top(i16 %a, i16 %b) {
b0:
  %r = call i16 @f(i16 %a)
  %w = zext i16 %r to i32
  %s = call i16 @f(i16 %b)
  %x = zext i16 %s to i32
  %t = add i32 %w, %x
  ret i32 %t
}
");
    assert!(after.contains("define internal zeroext i32 @f"), "{after}");
    assert!(after.contains("zext i16 %v to i32"), "{after}");
    assert!(after.matches("call zeroext i32 @f").count() == 2, "{after}");
    assert!(!after.contains("call i16 @f"), "{after}");
    assert_eq!(after.matches("zext").count(), 1, "the calls' extensions are gone:\n{after}");
}

/// A call whose result is read any other way keeps the narrow return.
#[test]
fn a_result_read_without_an_extension_keeps_the_function_as_it_is() {
    let before = "define internal i16 @f(i16 %n) {
b0:
  %v = shl i16 1, %n
  ret i16 %v
}
define i32 @top(i16 %a, i16 %b) {
b0:
  %r = call i16 @f(i16 %a)
  %w = zext i16 %r to i32
  %s = call i16 @f(i16 %b)
  %x = zext i16 %s to i32
  %y = add i16 %s, 1
  %z = zext i16 %y to i32
  %t = add i32 %w, %x
  %u = add i32 %t, %z
  ret i32 %u
}
";
    assert!(run(before).contains("define internal i16 @f"), "{}", run(before));
}

/// zext at one call and sext at another, or two widths, are two extensions.
#[test]
fn callers_that_extend_differently_keep_the_function_as_it_is() {
    let kinds = run("define internal i16 @f(i16 %n) {
b0:
  %v = shl i16 1, %n
  ret i16 %v
}
define i32 @top(i16 %a, i16 %b) {
b0:
  %r = call i16 @f(i16 %a)
  %w = zext i16 %r to i32
  %s = call i16 @f(i16 %b)
  %x = sext i16 %s to i32
  %t = add i32 %w, %x
  ret i32 %t
}
");
    assert!(kinds.contains("define internal i16 @f"), "{kinds}");
    let widths = run("define internal i16 @f(i16 %n) {
b0:
  %v = shl i16 1, %n
  ret i16 %v
}
define i64 @top(i16 %a, i16 %b) {
b0:
  %r = call i16 @f(i16 %a)
  %w = zext i16 %r to i32
  %s = call i16 @f(i16 %b)
  %x = zext i16 %s to i64
  %y = zext i32 %w to i64
  %t = add i64 %x, %y
  ret i64 %t
}
");
    assert!(widths.contains("define internal i16 @f"), "{widths}");
}

/// One call and one `ret`: the extension only moves, nothing is saved.
#[test]
fn one_call_of_one_return_is_left_alone() {
    let after = run("define internal i16 @f(i16 %n) {
b0:
  %v = shl i16 1, %n
  ret i16 %v
}
define i32 @top(i16 %a) {
b0:
  %r = call i16 @f(i16 %a)
  %w = zext i16 %r to i32
  ret i32 %w
}
");
    assert!(after.contains("define internal i16 @f"), "{after}");
}

/// A function other modules call keeps its type.
#[test]
fn an_exported_function_keeps_its_return_type() {
    let after = run("define i16 @f(i16 %n) {
b0:
  %v = shl i16 1, %n
  ret i16 %v
}
define i32 @top(i16 %a, i16 %b) {
b0:
  %r = call i16 @f(i16 %a)
  %w = zext i16 %r to i32
  %s = call i16 @f(i16 %b)
  %x = zext i16 %s to i32
  %t = add i32 %w, %x
  ret i32 %t
}
");
    assert!(after.contains("define i16 @f"), "{after}");
}

/// Sign extension is widened as zero extension is.
#[test]
fn a_sign_extension_at_every_call_is_made_in_the_function() {
    let after = run("define internal i8 @f(i8 %n) {
b0:
  %v = add i8 %n, 1
  ret i8 %v
}
define i16 @top(i8 %a, i8 %b) {
b0:
  %r = call i8 @f(i8 %a)
  %w = sext i8 %r to i16
  %s = call i8 @f(i8 %b)
  %x = sext i8 %s to i16
  %t = add i16 %w, %x
  ret i16 %t
}
");
    assert!(after.contains("define internal signext i16 @f") && after.contains("sext i8 %v to i16"), "{after}");
}
