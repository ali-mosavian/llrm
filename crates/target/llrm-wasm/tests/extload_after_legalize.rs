use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

const IR: &str = "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"
target triple = \"wasm32-unknown-unknown\"
declare { i32, i1 } @llvm.smul.with.overflow.i32(i32, i32)
define i32 @f(ptr %p, ptr %q, ptr %o) {
  %a = load i32, ptr %p
  %b = load i32, ptr %q
  %r = call { i32, i1 } @llvm.smul.with.overflow.i32(i32 %a, i32 %b)
  %v = extractvalue { i32, i1 } %r, 0
  %f = extractvalue { i32, i1 } %r, 1
  store i32 %v, ptr %o
  %z = zext i1 %f to i32
  ret i32 %z
}
";

/// The legalizer widens `smul.with.overflow.i32` to `sext` of both operands;
/// the loads were left as `i32.load` + `i64.extend_i32_s` because the
/// extending-load fold stopped before operations were legal (SD205). The pin
/// folds the `sext` of a load after legalization when the extending load is
/// legal: `i64.load32_s`. overflow_builtins was +4 B over the pin.
#[test]
fn an_extension_made_by_legalization_joins_its_load() {
    for level in [Level::O1, Level::O2] {
        let text = assembly(IR, Width::W32, Options { level, ..Options::default() }).unwrap();
        assert_eq!(text.matches("i64.load32_s").count(), 2, "{text}");
        assert!(!text.contains("i64.extend_i32_s"), "{text}");
    }
}
