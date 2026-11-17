//! A shuffle that fills lanes with zeros is `ZERO_EXTEND_VECTOR_INREG`
//! (`combineShuffleToZeroExtendVectorInReg`), which the loop vectorizer's
//! widened byte sums need: the pin's `ldr s; ushll; ushll; uaddlv`.
//! Before, the shuffle of a concat died in the legalizer ("ConcatVectors V4I8
//! promote operand") and the compile was refused.

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
fn a_shuffle_with_zeros_of_loaded_bytes_widens_with_ushll() {
    let asm = assembly(
        r#"
define i64 @f(ptr %q, i64 %acc) {
  %v = load <4 x i8>, ptr %q, align 1
  %ins = insertelement <4 x i8> poison, i8 0, i64 0
  %sh = shufflevector <4 x i8> %v, <4 x i8> %ins, <16 x i32> <i32 0, i32 4, i32 4, i32 4, i32 1, i32 4, i32 4, i32 4, i32 2, i32 4, i32 4, i32 4, i32 3, i32 4, i32 4, i32 4>
  %bc = bitcast <16 x i8> %sh to <4 x i32>
  %z = zext <4 x i32> %bc to <4 x i64>
  %r = call i64 @llvm.vector.reduce.add.v4i64(<4 x i64> %z)
  %s = add i64 %r, %acc
  ret i64 %s
}
declare i64 @llvm.vector.reduce.add.v4i64(<4 x i64>)
"#,
    );
    assert!(asm.contains("ushll"), "no widening shift:\n{asm}");
    assert!(!has_and(&asm), "the widened bytes are masked again:\n{asm}");
}

fn has_and(asm: &str) -> bool {
    asm.lines().any(|line| line.trim_start().starts_with("and v"))
}

/// `CombineExtLoad`: `zext (load <4 x i8>)` is one extending load that
/// `tryLowerSmallVectorExtLoad` widens with `ushll`s. Before, the `v4i8` was
/// promoted, any-extended and masked: `ushll; movi; ushll; and`.
#[test]
fn a_zext_of_a_small_vector_load_is_not_masked() {
    let asm = assembly(
        r#"
define void @f(ptr %q, ptr %o) {
  %v = load <4 x i8>, ptr %q, align 1
  %z = zext <4 x i8> %v to <4 x i32>
  store <4 x i32> %z, ptr %o
  ret void
}
"#,
    );
    assert_eq!(asm.matches("ushll").count(), 2, "{asm}");
    assert!(!has_and(&asm), "{asm}");
}

/// Two-fold widening is `LowerZERO_EXTEND_VECTOR_INREG`: `zip1` with zeros
/// (the pin: `movi; ldr; zip1 v0.8h`).
#[test]
fn a_shuffle_that_interleaves_zeros_is_zip1() {
    let asm = assembly(
        r#"
define void @f(ptr %q, ptr %o) {
  %v = load <8 x i16>, ptr %q
  %s = shufflevector <8 x i16> %v, <8 x i16> zeroinitializer, <8 x i32> <i32 0, i32 8, i32 1, i32 8, i32 2, i32 8, i32 3, i32 8>
  %b = bitcast <8 x i16> %s to <4 x i32>
  store <4 x i32> %b, ptr %o
  ret void
}
"#,
    );
    assert!(asm.contains("zip1 v") && asm.contains(".8h"), "no zip1:\n{asm}");
}
