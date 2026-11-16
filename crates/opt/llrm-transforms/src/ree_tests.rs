//! Tests of `ree`: the extension of a loop-carried narrow value, made on the
//! arms of its phi.

use llrm_analysis::testing::DOS;

use super::Ree;
use crate::testing::{Tuned, managed_on, parsed};

fn run(body: &str) -> String {
    managed_on(&mut parsed(&format!("{DOS}{body}")), Ree, Tuned { ext_loads_free: true, ..Default::default() })
}

/// The descent of phi_slot_unknown_arm: a `short` loaded in the latch, carried
/// round the loop and sign-extended at the top of every trip (`movzx ebx, word
/// [m]` then `movsx ebx, bx`: gcc's `ree` loads it with `movsx` once).
#[test]
fn test_a_loop_carried_loaded_short_is_extended_once() {
    let after = run("define i32 @f(ptr %t) {
b0:
  br label %b1
b1:
  %p = phi i16 [ 1, %b0 ], [ %v, %b1 ]
  %e = sext i16 %p to i32
  %q = getelementptr i16, ptr %t, i32 %e
  %v = load i16, ptr %q
  %z = icmp eq i16 %v, 0
  br i1 %z, label %b2, label %b1
b2:
  ret i32 %e
}
");
    assert!(after.contains("phi i32 [ 1, %b0 ], [ %") && !after.contains("phi i16"), "{after}");
    assert!(after.contains("sext i16 %v to i32"), "{after}");
}

/// A zero extension takes the constant arm zero-extended, not sign-extended.
#[test]
fn test_a_zero_extended_constant_arm_keeps_its_high_bit_clear() {
    let after = run("define i32 @f(ptr %t) {
b0:
  br label %b1
b1:
  %p = phi i16 [ -1, %b0 ], [ %v, %b1 ]
  %e = zext i16 %p to i32
  %q = getelementptr i16, ptr %t, i32 %e
  %v = load i16, ptr %q
  %z = icmp eq i16 %v, 0
  br i1 %z, label %b2, label %b1
b2:
  ret i32 %e
}
");
    assert!(after.contains("phi i32 [ 65535, %b0 ]"), "{after}");
}

/// Another reader of the narrow phi would keep it live beside the wide one:
/// left.
#[test]
fn test_a_phi_with_a_narrow_reader_is_left() {
    let after = run("define i32 @f(ptr %t) {
b0:
  br label %b1
b1:
  %p = phi i16 [ 1, %b0 ], [ %v, %b1 ]
  %e = sext i16 %p to i32
  %q = getelementptr i16, ptr %t, i32 %e
  %v = load i16, ptr %q
  %z = icmp eq i16 %p, 0
  br i1 %z, label %b2, label %b1
b2:
  ret i32 %e
}
");
    assert!(after.contains("phi i16"), "{after}");
}

/// An arm that is not a load or a constant (an add) takes the extension only
/// after it: left.
#[test]
fn test_an_arm_that_is_an_add_is_left() {
    let after = run("define i32 @f(i16 %x) {
b0:
  br label %b1
b1:
  %p = phi i16 [ 1, %b0 ], [ %v, %b1 ]
  %e = sext i16 %p to i32
  %v = add i16 %p, %x
  %z = icmp eq i32 %e, 100
  br i1 %z, label %b2, label %b1
b2:
  ret i32 %e
}
");
    assert!(after.contains("phi i16"), "{after}");
}

/// A target with no extending load (m68k) keeps the narrow phi: the extension
/// moved onto the load was a `move.w`, an `ext.l` and a `tst.l` on
/// phi_slot_unknown_arm, one more instruction a trip than before.
#[test]
fn test_a_target_without_an_extending_load_leaves_the_phi() {
    let after = managed_on(
        &mut parsed(&format!(
            "{DOS}define i32 @f(ptr %t) {{
b0:
  br label %b1
b1:
  %p = phi i16 [ 1, %b0 ], [ %v, %b1 ]
  %e = sext i16 %p to i32
  %q = getelementptr i16, ptr %t, i32 %e
  %v = load i16, ptr %q
  %z = icmp eq i16 %v, 0
  br i1 %z, label %b2, label %b1
b2:
  ret i32 %e
}}
"
        )),
        Ree,
        Tuned::default(),
    );
    assert!(after.contains("phi i16") && !after.contains("phi i32"), "{after}");
}
