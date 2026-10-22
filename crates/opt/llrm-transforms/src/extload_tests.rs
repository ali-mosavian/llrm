//! Tests of `extload`: a load and its extension, and the compares of the load.

use llrm_analysis::testing::DOS;

use super::ExtLoad;
use crate::testing::{managed, parsed};

fn run(body: &str) -> String {
    managed(&mut parsed(&format!("{DOS}{body}")), ExtLoad)
}

/// `*p > 0` then `t[*p]`: the byte was compared on a byte register and
/// extended again for the index (`mov al, [si]; test al, al; ... movsx si, al`,
/// +1 instruction a function: QCport keybind, ls, r_bsp). The pin makes one
/// `movsx` from memory and compares the wide value.
#[test]
fn test_a_compare_of_a_loaded_byte_compares_its_extension() {
    let after = run("define i8 @f(ptr %p, ptr %t) {
b0:
  %c = load i8, ptr %p
  %lt = icmp sgt i8 %c, 0
  %w = sext i8 %c to i16
  br i1 %lt, label %b1, label %b2
b1:
  %q = getelementptr i8, ptr %t, i16 %w
  %v = load i8, ptr %q
  ret i8 %v
b2:
  ret i8 0
}
");
    assert!(after.contains("icmp sgt i16 %w, 0"), "{after}");
    assert!(!after.contains("icmp sgt i8"), "{after}");
}

/// A `zext` loses the sign bits a signed compare reads: the compare stays on
/// the byte.
#[test]
fn test_a_zero_extension_is_not_made_for_a_signed_compare() {
    let after = run("define i8 @f(ptr %p, ptr %t) {
b0:
  %c = load i8, ptr %p
  %lt = icmp sgt i8 %c, 0
  %w = zext i8 %c to i16
  br i1 %lt, label %b1, label %b2
b1:
  %q = getelementptr i8, ptr %t, i16 %w
  %v = load i8, ptr %q
  ret i8 %v
b2:
  ret i8 0
}
");
    assert!(after.contains("icmp sgt i8 %c, 0"), "{after}");
}

/// A `zext` and an equality or unsigned compare: the constant is zero
/// extended.
#[test]
fn test_an_unsigned_compare_of_a_zero_extended_load_takes_the_wide_constant() {
    let after = run("define i8 @f(ptr %p, ptr %t) {
b0:
  %c = load i8, ptr %p
  %lt = icmp ult i8 %c, 200
  %w = zext i8 %c to i16
  br i1 %lt, label %b1, label %b2
b1:
  %q = getelementptr i8, ptr %t, i16 %w
  %v = load i8, ptr %q
  ret i8 %v
b2:
  ret i8 0
}
");
    assert!(after.contains("icmp ult i16 %w, 200"), "{after}");
}

/// A sign-extended negative constant stays the same number.
#[test]
fn test_a_negative_constant_is_sign_extended() {
    let after = run("define i8 @f(ptr %p, ptr %t) {
b0:
  %c = load i8, ptr %p
  %lt = icmp eq i8 %c, -1
  %w = sext i8 %c to i16
  br i1 %lt, label %b1, label %b2
b1:
  %q = getelementptr i8, ptr %t, i16 %w
  %v = load i8, ptr %q
  ret i8 %v
b2:
  ret i8 0
}
");
    assert!(after.contains("icmp eq i16 %w, -1"), "{after}");
}

/// Another reader of the byte (here a store) leaves the load alone: reading
/// `trunc E` kept the narrow and the wide value live together (grep.nib
/// `_main`, +58 B).
#[test]
fn test_a_load_with_another_reader_is_left() {
    let text = "define i8 @f(ptr %p, ptr %t, ptr %s) {
b0:
  %c = load i8, ptr %p
  %lt = icmp sgt i8 %c, 0
  %w = sext i8 %c to i16
  store i8 %c, ptr %s
  %q = getelementptr i8, ptr %t, i16 %w
  %v = load i8, ptr %q
  ret i8 %v
}
";
    let mut module = parsed(&format!("{DOS}{text}"));
    let before = crate::testing::printed(&module);
    assert_eq!(managed(&mut module, ExtLoad), before);
}

/// A load with no extension is left.
#[test]
fn test_a_load_without_an_extension_is_left() {
    let text = "define i1 @f(ptr %p) {
b0:
  %c = load i8, ptr %p
  %lt = icmp sgt i8 %c, 0
  ret i1 %lt
}
";
    let mut module = parsed(&format!("{DOS}{text}"));
    let before = crate::testing::printed(&module);
    assert_eq!(managed(&mut module, ExtLoad), before);
}
