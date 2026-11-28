use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

const IR: &str = "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"
target triple = \"wasm32-unknown-unknown\"
define float @lo(float %x, float %y) {
  %c = fcmp olt float %x, %y
  %s = select i1 %c, float %x, float %y
  ret float %s
}
define double @hi(double %x, double %y) {
  %c = fcmp ogt double %x, %y
  %s = select i1 %c, double %x, double %y
  ret double %s
}
";

/// `combineSelectToPseudoMinMax` saw a wasm float select (kept for the DAG
/// since #7356) and made a `PseudoFMin`/`PseudoFMax` nobody lowers: jpeg_single
/// and stft_single stopped with `CannotSelect("PseudoFMin")`. The node is
/// Expand by default (TargetLoweringBase.cpp#L864).
#[test]
fn a_float_clamp_compiles_where_there_is_no_minss() {
    for level in [Level::O0, Level::O2] {
        let text = assembly(IR, Width::W32, Options { level, ..Options::default() }).unwrap();
        assert!(text.contains("f32.lt") && text.contains("f64.gt"), "{text}");
    }
}
