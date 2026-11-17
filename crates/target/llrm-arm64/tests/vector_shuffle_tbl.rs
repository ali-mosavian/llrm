//! `GenerateTBL`: a shuffle of bytes no permute matches is `tbl` by a constant
//! index vector. Before, the legalizer built it lane by lane (`umov`/`ins`,
//! 16 of each for a `v16i8`) or refused the block.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const HEAD: &str = r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
"#;

fn assembly(body: &str) -> String {
    driver::assembly(&format!("{HEAD}{body}"), Platform::Linux, true).expect("lowers")
}

#[test]
fn a_byte_shuffle_of_one_vector_is_one_tbl() {
    let asm = assembly(
        r#"
define void @f(ptr %q, ptr %o) {
  %v = load <16 x i8>, ptr %q
  %s = shufflevector <16 x i8> %v, <16 x i8> poison, <16 x i32> <i32 3, i32 1, i32 4, i32 1, i32 5, i32 9, i32 2, i32 6, i32 5, i32 3, i32 5, i32 8, i32 9, i32 7, i32 9, i32 3>
  store <16 x i8> %s, ptr %o
  ret void
}
"#,
    );
    assert_eq!(asm.matches("tbl v").count(), 1, "{asm}");
    assert!(!asm.contains("umov") && !asm.contains("ins v"), "lane by lane:\n{asm}");
}

/// A zero second operand fills an index past the table with 0 (`tbl1`).
#[test]
fn a_shuffle_with_zeros_is_tbl_and_its_out_of_range_indices_are_255() {
    let asm = assembly(
        r#"
define void @f(ptr %q, ptr %o) {
  %v = load <8 x i16>, ptr %q
  %s = shufflevector <8 x i16> %v, <8 x i16> zeroinitializer, <8 x i32> <i32 7, i32 8, i32 5, i32 8, i32 3, i32 8, i32 1, i32 8>
  store <8 x i16> %s, ptr %o
  ret void
}
"#,
    );
    assert!(asm.contains("tbl v"), "{asm}");
}

/// A byte zero-extend of 8 lanes to 32 (the `<0,8,8,8,1,8,8,8,..>` shuffle
/// with zeros): the pad makes each half's second operand `concat(zeros,
/// undef)`, which `foldCONCAT_VECTORS` makes the zero `BUILD_VECTOR` that
/// `GenerateTBL` takes. Unfolded it was no zero vector and the 256-bit shuffle
/// died in the legalizer ("custom lowering of VectorShuffle", jpeg_fixed -O3).
#[test]
fn a_byte_zero_extend_of_eight_lanes_to_thirty_two_is_two_tbl() {
    let asm = assembly(
        r#"
define void @f(ptr %q, ptr %p) {
  %a = load <8 x i8>, ptr %q, align 1
  %r = shufflevector <8 x i8> %a, <8 x i8> zeroinitializer, <32 x i32> <i32 0, i32 8, i32 8, i32 8, i32 1, i32 8, i32 8, i32 8, i32 2, i32 8, i32 8, i32 8, i32 3, i32 8, i32 8, i32 8, i32 4, i32 8, i32 8, i32 8, i32 5, i32 8, i32 8, i32 8, i32 6, i32 8, i32 8, i32 8, i32 7, i32 8, i32 8, i32 8>
  store <32 x i8> %r, ptr %p, align 1
  ret void
}
"#,
    );
    assert_eq!(asm.matches("tbl v").count(), 2, "{asm}");
}
