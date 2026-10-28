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

/// A reader in the load's own block (here a store) reads the truncated
/// extension: the load has one reader, so selection folds it into the extending
/// load.
#[test]
fn test_a_reader_in_the_loads_block_reads_the_truncated_extension() {
    let after = run("define i8 @f(ptr %p, ptr %t, ptr %s) {
b0:
  %c = load i8, ptr %p
  %lt = icmp sgt i8 %c, 0
  %w = sext i8 %c to i16
  store i8 %c, ptr %s
  %q = getelementptr i8, ptr %t, i16 %w
  %v = load i8, ptr %q
  ret i8 %v
}
");
    assert!(after.contains("icmp sgt i16 %w, 0") && after.contains("trunc i16 %w to i8"), "{after}");
}

/// A reader in another block (here a store) keeps the load as it is: reading
/// `trunc E` there kept the narrow and the wide value live together
/// (grep.nib `_main`).
#[test]
fn test_a_reader_in_another_block_leaves_the_load_alone() {
    let text = "define i8 @f(ptr %p, ptr %t, ptr %s, i1 %c) {
b0:
  %b = load i8, ptr %p
  %lt = icmp sgt i8 %b, 0
  %w = sext i8 %b to i16
  br i1 %c, label %b1, label %b2
b1:
  store i8 %b, ptr %s
  %q = getelementptr i8, ptr %t, i16 %w
  %v = load i8, ptr %q
  ret i8 %v
b2:
  ret i8 0
}
";
    let mut module = parsed(&format!("{DOS}{text}"));
    let before = crate::testing::printed(&module);
    assert_eq!(managed(&mut module, ExtLoad), before);
}

/// A compare with a constant in a later block compares the extension too, as
/// the one in the load's block does (x_life's neighbour count tested in the
/// loop's next block: 16 B of `movzx`).
#[test]
fn test_a_compare_in_a_later_block_compares_the_extension() {
    let after = run("define i8 @f(ptr %p, ptr %t, i1 %c) {
b0:
  %b = load i8, ptr %p
  %w = zext i8 %b to i16
  br i1 %c, label %b1, label %b2
b1:
  %nz = icmp ne i8 %b, 0
  %q = getelementptr i8, ptr %t, i16 %w
  %v = load i8, ptr %q
  %r = select i1 %nz, i8 %v, i8 1
  ret i8 %r
b2:
  ret i8 0
}
");
    assert!(after.contains("icmp ne i16 %w, 0") && !after.contains("icmp ne i8"), "{after}");
}

/// The extension made again in another block is the first one.
#[test]
fn test_the_same_extension_made_again_in_another_block_is_the_first() {
    let after = run("define i16 @f(ptr %p, i1 %c) {
b0:
  %b = load i8, ptr %p
  %w = zext i8 %b to i16
  %nz = icmp ne i8 %b, 0
  br i1 %nz, label %b1, label %b2
b1:
  %again = zext i8 %b to i16
  %s = add i16 %w, %again
  ret i16 %s
b2:
  ret i16 %w
}
");
    assert_eq!(after.matches("zext").count(), 1, "{after}");
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

/// A block of N loads nothing extends: each load scanned the rest of the block
/// for an extension, N^2/2 steps (`mir extload` at `equal` N=8192: 5882 Minstr,
/// x4.5 a doubling). The readers of a load name its extensions.
#[test]
fn test_a_load_nothing_extends_does_not_scan_the_block() {
    let n = 400;
    let mut body = String::from("define i32 @f(ptr %p) {\nb0:\n");
    for k in 0..n {
        body.push_str(&format!("  %g{k} = getelementptr i32, ptr %p, i16 {k}\n  %v{k} = load i32, ptr %g{k}\n"));
    }
    body.push_str("  %s = add i32 %v0, %v1\n  ret i32 %s\n}\n");
    super::SCANNED.with(|one| one.set(0));
    let after = run(&body);
    assert!(after.contains("load i32"), "{after}");
    let steps = super::SCANNED.with(std::cell::Cell::get);
    assert!(steps <= 4 * n, "{steps} instructions scanned for {n} loads");
}

/// Several extensions of one load in its block: the compare takes the earliest,
/// as the block scan chose it, whichever the readers list first.
#[test]
fn test_the_earliest_of_several_extensions_of_a_load_takes_the_compare() {
    for (first, second, wide) in
        [("zext i8 %c to i16", "sext i8 %c to i32", "i16"), ("sext i8 %c to i32", "zext i8 %c to i16", "i32")]
    {
        let after = run(&format!(
            "define i32 @f(ptr %p) {{
b0:
  %c = load i8, ptr %p
  %lt = icmp eq i8 %c, 3
  %a = {first}
  %b = {second}
  br i1 %lt, label %b1, label %b2
b1:
  ret i32 1
b2:
  ret i32 0
}}
"
        ));
        assert!(after.contains(&format!("icmp eq {wide} %a, 3")), "{after}");
    }
}
