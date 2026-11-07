use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

/// A `mul` stored as a byte demands only its low 8 bits, so the `and 65535`
/// the i16 `lshr` promoted to is redundant: histo -O2 kept an `i32.and` with
/// 65504 before `i32.shr_u` (the pin has none), 2 instructions and 4 bytes a
/// loop.
#[test]
fn a_byte_stored_product_drops_the_mask_of_its_shifted_operand() {
    let ir = "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"
target triple = \"wasm32-unknown-unknown\"
define void @f(i32 %x, ptr %p) {
  %t = trunc i32 %x to i16
  %a = and i16 %t, 127
  %s = lshr i16 %t, 5
  %m = mul i16 %a, %s
  %r = trunc i16 %m to i8
  store i8 %r, ptr %p
  ret void
}
";
    let text = assembly(ir, Width::W32, Options { level: Level::O2, ..Options::default() }).unwrap();
    assert_eq!(text.matches("i32.and").count(), 1, "{text}");
}
