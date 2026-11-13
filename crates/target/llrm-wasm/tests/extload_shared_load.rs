use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

const IR: &str = "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"
target triple = \"wasm32-unknown-unknown\"
define i64 @f(ptr %p, ptr %o) {
  %a = load i32, ptr %p
  %z = zext i32 %a to i64
  store i32 %a, ptr %o
  ret i64 %z
}
";

/// A `zext` of a load that a store also reads was folded into `i64.load32_u`
/// with the store taking a `i32.wrap_i64` of it. The pin's
/// `ExtendUsesToFormExtLoad` refuses when truncates are not free (wasm's
/// are not) and a reader is not a compare: `i32.load` once, `i64.extend_i32_u`.
/// wide_shift_by_one was +4 B over the pin at -O2, -O3 and -Os.
#[test]
fn a_load_another_reader_takes_narrow_is_not_widened_for_a_zext() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let text = assembly(IR, Width::W32, Options { level, ..Options::default() }).unwrap();
        assert!(!text.contains("i64.load32_u"), "{text}");
        assert!(text.contains("i64.extend_i32_u"), "{text}");
    }
}
