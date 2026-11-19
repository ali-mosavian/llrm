use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

const IR: &str = "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"
target triple = \"wasm32-unknown-unknown\"
declare i32 @llvm.bswap.i32(i32)
declare i64 @llvm.bswap.i64(i64)
define i32 @swap32(i32 %x) {
  %r = call i32 @llvm.bswap.i32(i32 %x)
  ret i32 %r
}
define i64 @swap64(i64 %x) {
  %r = call i64 @llvm.bswap.i64(i64 %x)
  ret i64 %r
}
";

/// wasm has no byte swap: `llvm.bswap.i32` and `.i64` stopped the compile with
/// "convert node to libcall" (stft_single, stft_fixed -O2 and -Os failed to
/// build while the pin builds them). `expandBSWAP` makes rotates and xors
/// (`i32`) or shifts and masks (`i64`).
#[test]
fn byte_swaps_of_words_expand_instead_of_stopping_the_compile() {
    for level in [Level::O0, Level::O2] {
        let text = assembly(IR, Width::W32, Options { level, ..Options::default() }).unwrap();
        let body = |name: &str| {
            let start = text.find(&format!("{name}:")).expect("the function");
            text[start..].split("end_function").next().unwrap().to_string()
        };
        // wasm has `rotr`, so the pin takes `i32` through the xor form.
        assert!(body("swap32").contains("i32.rotr") && body("swap32").contains("i32.xor"), "{text}");
        assert!(body("swap64").contains("i64.shl") && body("swap64").contains("i64.shr_u"), "{text}");
    }
}

/// `cttz` and `ctlz` of a word that may be zero reached the legalizer as
/// `CTTZ_ZERO_UNDEF`, which wasm expands to `ctz` (the pin's `expandCTTZ`);
/// ours stopped at "convert node to libcall".
#[test]
fn count_zeros_with_a_poison_zero_expand_to_the_plain_count() {
    let ir = "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"
target triple = \"wasm32-unknown-unknown\"
declare i32 @llvm.cttz.i32(i32, i1)
declare i32 @llvm.ctlz.i32(i32, i1)
define i32 @tz(i32 %x) {
  %r = call i32 @llvm.cttz.i32(i32 %x, i1 true)
  ret i32 %r
}
define i32 @lz(i32 %x) {
  %r = call i32 @llvm.ctlz.i32(i32 %x, i1 true)
  ret i32 %r
}
";
    let text = assembly(ir, Width::W32, Options { level: Level::O0, ..Options::default() }).unwrap();
    assert!(text.contains("i32.ctz") && text.contains("i32.clz"), "{text}");
}
