//! Each body is MIR text run by llrm-mir's interpreter before and after, so a
//! wrong reduction shows as a wrong answer; what is asserted of the text is
//! the width the graph is left at.

use llrm_analysis::testing::DOS;

use super::TruncInstCombine;
use crate::testing::{managed, parsed, results};

/// `text` through the pass: its printed form, run as before on `inputs`.
fn reduced(
    text: &str,
    inputs: &[&[i128]],
) -> String {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let after = managed(&mut module, TruncInstCombine);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    after
}

const INPUTS: &[&[i128]] = &[&[0, 0], &[1, 2], &[200, 100], &[255, 255], &[7, 250]];

const ADD: &str = "define i8 @f(i8 %x, i8 %y) {
b0:
  %a = zext i8 %x to i32
  %b = zext i8 %y to i32
  %s = add i32 %a, %b
  %c = trunc i32 %s to i8
  ret i8 %c
}
";

/// `trunc(zext(x) + zext(y))` is `x + y` at the narrow width, and no `i32`.
#[test]
fn an_add_of_extended_bytes_truncated_is_an_add_of_bytes() {
    let after = reduced(ADD, INPUTS);
    assert!(!after.contains("zext") && !after.contains("trunc"), "{after}");
    assert!(after.contains("add i8 %x, %y"), "{after}");
}

/// A shift by a constant keeps the bits that move down: `lshr` by 3 of a byte
/// needs only the byte.
#[test]
fn a_shift_by_a_constant_of_an_extended_byte_is_a_shift_of_the_byte() {
    let text = ADD.replace("%s = add i32 %a, %b", "%s = lshr i32 %a, 3");
    let after = reduced(&text, INPUTS);
    assert!(after.contains("lshr i8 %x, 3"), "{after}");
}

/// The bits `lshr` brings down from above the narrow width are not zero here:
/// `x >> 4` of a 32-bit unknown, truncated, stays 32 bits wide.
#[test]
fn a_shift_of_an_unknown_wide_value_is_left_alone() {
    let text = "define i8 @f(i32 %x, i32 %y) {
b0:
  %s = lshr i32 %x, 4
  %c = trunc i32 %s to i8
  ret i8 %c
}
";
    let after = reduced(text, &[&[0x1234, 0], &[-1, 0]]);
    assert!(after.contains("lshr i32"), "{after}");
}

/// A use of the sum outside the graph keeps it as it is: no instruction is
/// made twice.
#[test]
fn a_sum_used_elsewhere_is_left_alone() {
    let text = "@g = global i32 0

define i8 @f(i8 %x, i8 %y) {
b0:
  %a = zext i8 %x to i32
  %b = zext i8 %y to i32
  %s = add i32 %a, %b
  store i32 %s, ptr @g
  %c = trunc i32 %s to i8
  ret i8 %c
}
";
    let after = reduced(text, INPUTS);
    assert!(after.contains("add i32"), "{after}");
}

/// A cycle through a phi is reduced with its new phi: the byte counter that
/// is widened for the add and truncated for the return.
#[test]
fn a_phi_cycle_is_reduced_to_a_phi_of_the_narrow_width() {
    let text = "define i8 @f(i8 %x, i8 %n) {
b0:
  br label %h

h:
  %i = phi i32 [ 0, %b0 ], [ %j, %h ]
  %k = phi i32 [ 0, %b0 ], [ %m, %h ]
  %e = zext i8 %x to i32
  %m = add i32 %k, %e
  %j = add i32 %i, 1
  %c = icmp ult i32 %j, 3
  br i1 %c, label %h, label %x

x:
  %t = trunc i32 %m to i8
  ret i8 %t
}
";
    let after = reduced(text, &[&[1, 0], &[100, 0], &[255, 0]]);
    assert!(after.contains("phi i8"), "{after}");
    assert!(!after.contains("%e = zext"), "{after}");
}

/// A chain of `n` additions of extended bytes, truncated once.
fn chain(n: usize) -> String {
    let mut text = String::from("define i8 @f(i8 %x) {\nb0:\n  %e = zext i8 %x to i32\n  %s0 = add i32 %e, 1\n");
    for k in 0..n {
        text += &format!("  %s{} = add i32 %s{k}, %e\n", k + 1);
    }
    text + &format!("  %c = trunc i32 %s{n} to i8\n  ret i8 %c\n}}\n")
}

/// The pass is linear in the chain: four times the length costs about four
/// times, not sixteen (a slope test at 1x and 4x, the least of three runs).
#[test]
fn the_time_grows_with_the_chain_not_with_its_square() {
    let least = |n: usize| {
        let text = chain(n);
        (0..3)
            .map(|_| {
                let mut module = parsed(&format!("{DOS}{text}"));
                llrm_support::debug::work_of(|| managed(&mut module, TruncInstCombine)).1 as f64
            })
            .fold(f64::MAX, f64::min)
    };
    let (small, large) = (least(500), least(2000));
    assert!(large < small * 6.0, "500 adds {small:.0} work, 2000 adds {large:.0}: a ratio of {:.1}", large / small);
}

/// An extension from the narrow type with a user outside the graph is kept for
/// that user; the graph is still reduced, and the extension is not made twice.
#[test]
fn an_extension_from_the_narrow_type_with_another_user_stays_for_it() {
    let text = "@g = global i32 0

define i8 @f(i8 %x, i8 %y) {
b0:
  %a = zext i8 %x to i32
  store i32 %a, ptr @g
  %b = zext i8 %y to i32
  %s = add i32 %a, %b
  %c = trunc i32 %s to i8
  ret i8 %c
}
";
    let after = reduced(text, INPUTS);
    assert!(after.contains("add i8 %x, %y"), "{after}");
    assert_eq!(after.matches("zext").count(), 1, "{after}");
}
