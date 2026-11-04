//! Each body is MIR text run by llrm-mir's interpreter before and after, so a
//! wrong merge shows as a wrong answer; what is asserted of the text is the
//! loads left.

use llrm_analysis::testing::DOS;

use super::AggressiveInstCombine;
use crate::testing::{managed, parsed, results};

fn merged(text: &str) -> String {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    managed(&mut module, AggressiveInstCombine);
    // The pin leaves the old chain to dead-code elimination.
    let after = managed(&mut module, crate::dead::Dead);
    let inputs: &[&[i128]] = &[&[0], &[1]];
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    after
}

const BYTES: &str = "@g = global [4 x i8] zeroinitializer
define i32 @f() {
b0:
  %p1 = getelementptr i8, ptr @g, i32 1
  %p2 = getelementptr i8, ptr @g, i32 2
  %p3 = getelementptr i8, ptr @g, i32 3
  %b0 = load i8, ptr @g
  %b1 = load i8, ptr %p1
  %b2 = load i8, ptr %p2
  %b3 = load i8, ptr %p3
  %z0 = zext i8 %b0 to i32
  %z1 = zext i8 %b1 to i32
  %z2 = zext i8 %b2 to i32
  %z3 = zext i8 %b3 to i32
  %s1 = shl i32 %z1, 8
  %s2 = shl i32 %z2, 16
  %s3 = shl i32 %z3, 24
  %o1 = or i32 %z0, %s1
  %o2 = or i32 %o1, %s2
  %o3 = or i32 %o2, %s3
  ret i32 %o3
}
";

/// Four byte loads or'd with shifts stayed four `movzx`; the pin makes them
/// one `load i32`.
#[test]
fn four_adjacent_byte_loads_assembled_little_endian_are_one_dword_load() {
    let after = merged(BYTES);
    assert_eq!(after.matches("load ").count(), 1, "{after}");
    assert!(after.contains("load i32"), "{after}");
}

/// A store between the loads may change a byte: no merge.
#[test]
fn a_store_between_the_loads_stops_the_merge() {
    let text = BYTES.replace("  %b1 = load", "  store i8 1, ptr @g\n  %b1 = load");
    let after = merged(&text);
    assert_eq!(after.matches("load ").count(), 4, "{after}");
}

/// Bytes in the wrong order are not a little-endian dword.
#[test]
fn bytes_out_of_order_are_not_merged() {
    let text = BYTES.replace("shl i32 %z1, 8", "shl i32 %z1, 16").replace("shl i32 %z2, 16", "shl i32 %z2, 8");
    let after = merged(&text);
    assert_eq!(after.matches("load ").count(), 4, "{after}");
}
