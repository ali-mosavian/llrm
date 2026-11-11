use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

const IR: &str = "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"
target triple = \"wasm32-unknown-unknown\"
@vf = global [2 x float] zeroinitializer
@vd = global [2 x double] zeroinitializer
define void @f() {
  store volatile float -1.0, ptr @vf, align 4
  store volatile double -1.0, ptr @vd, align 8
  ret void
}
";

/// A volatile `float`/`double` constant store stayed `f32.const` + `f32.store`
/// (`f64` likewise) because the wasm combiner left `store_fp_constant_as_i64`
/// off; the pin's `replaceStoreOfFPConstant` makes it an integer store, so
/// fcmp_all was +5 B over the pin at every level.
#[test]
fn volatile_float_constant_stores_are_integer_stores() {
    for level in [Level::O1, Level::O2] {
        let text = assembly(IR, Width::W32, Options { level, ..Options::default() }).unwrap();
        assert!(text.contains("i32.store") && text.contains("i64.store"), "{text}");
        assert!(!text.contains("f32.store") && !text.contains("f64.store"), "{text}");
    }
}
