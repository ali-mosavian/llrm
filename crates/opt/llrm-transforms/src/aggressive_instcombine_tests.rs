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
    let inputs: &[&[i128]] = &[&[0], &[1], &[4660]];
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

const HALVES: &str = "@g = global [4 x i8] zeroinitializer
define i16 @f(i16 %x) {
b0:
  %lo = trunc i16 %x to i8
  %h = lshr i16 %x, 8
  %hi = trunc i16 %h to i8
  store i8 %lo, ptr @g
  %p = getelementptr i8, ptr @g, i16 1
  store i8 %hi, ptr %p
  %r = load i16, ptr @g
  ret i16 %r
}
";

/// Two byte stores of the halves of one value stayed two `mov`s; the pin
/// makes them one `store i16`.
#[test]
fn the_two_bytes_of_a_value_stored_side_by_side_are_one_word_store() {
    let after = merged(HALVES);
    assert_eq!(after.matches("store ").count(), 1, "{after}");
    assert!(after.contains("store i16"), "{after}");
}

/// A load of the object between the stores may see the first alone: no merge.
#[test]
fn a_read_between_the_stores_stops_the_merge() {
    let text = HALVES.replace("  %p = getelementptr", "  %m = load i8, ptr @g\n  %p = getelementptr");
    let after = merged(&text);
    assert_eq!(after.matches("store ").count(), 2, "{after}");
}

/// Bytes two apart leave a gap the wider store would write.
#[test]
fn stores_with_a_gap_between_them_are_not_merged() {
    let text = HALVES.replace("i16 1\n  store", "i16 2\n  store");
    let after = merged(&text);
    assert_eq!(after.matches("store ").count(), 2, "{after}");
}

const POPCOUNT: &str = "define i32 @f(i32 %x) {
b0:
  %a = lshr i32 %x, 1
  %b = and i32 %a, 1431655765
  %c = sub i32 %x, %b
  %d = and i32 %c, 858993459
  %e = lshr i32 %c, 2
  %f = and i32 %e, 858993459
  %g = add i32 %d, %f
  %h = lshr i32 %g, 4
  %i = add i32 %g, %h
  %j = and i32 %i, 252645135
  %k = mul i32 %j, 16843009
  %l = lshr i32 %k, 24
  ret i32 %l
}
";

/// The parallel bit count (Seander Anderson's) stayed twelve instructions and
/// a multiply; the pin makes it `llvm.ctpop`.
#[test]
fn the_parallel_bit_count_is_a_ctpop() {
    let after = merged(POPCOUNT);
    assert!(after.contains("llvm.ctpop.i32"), "{after}");
    assert!(!after.contains("mul "), "{after}");
}

/// A mask that is not 0x55... is not the idiom.
#[test]
fn a_bit_count_with_another_mask_is_left_alone() {
    let after = merged(&POPCOUNT.replace("1431655765", "1431655764"));
    assert!(!after.contains("llvm.ctpop"), "{after}");
}
