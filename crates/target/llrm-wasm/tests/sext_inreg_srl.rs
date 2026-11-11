use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

const IR: &str = "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"
target triple = \"wasm32-unknown-unknown\"
define i32 @signed_high(i16 signext %x) {
  %s = lshr i16 %x, 8
  %z = zext nneg i16 %s to i32
  %l = shl nuw i32 %z, 24
  %r = ashr exact i32 %l, 24
  ret i32 %r
}
";

/// `(signed char)(short x >> 8)` was `shr_u 8` + `extend8_s`: the `(sext_inreg
/// (srl X, 8), i8)` fold to `(sra X, 8)` was missing, so high_byte was +1 B
/// over the pin (`shr_s 8`) at every level.
#[test]
fn a_signed_byte_of_a_shifted_sign_extended_value_is_one_arithmetic_shift() {
    for level in [Level::O1, Level::O2] {
        let text = assembly(IR, Width::W32, Options { level, ..Options::default() }).unwrap();
        assert!(text.contains("i32.shr_s") && !text.contains("i32.extend8_s") && !text.contains("i32.shr_u"), "{text}");
    }
}
