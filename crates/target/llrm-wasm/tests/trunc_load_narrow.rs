use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

const IR: &str = "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"
target triple = \"wasm32-unknown-unknown\"
@table = hidden global [3 x i64] [i64 1099511627776, i64 -2, i64 77], align 16
declare void @report(i32)
define i32 @f() {
  %b = load i64, ptr getelementptr inbounds nuw (i8, ptr @table, i32 8), align 8
  %u = trunc i64 %b to i32
  call void @report(i32 1)
  ret i32 %u
}
";

/// A call after the load reads the load's chain, and the narrowing counted
/// that read as a second use of the loaded value, so `trunc (load i64)` stayed
/// `i64.load` + `i32.wrap_i64`. The pin counts uses of the value alone
/// (`isLegalNarrowLdSt`: `SDValue(Load, 0).hasOneUse()`): `i32.load`.
/// data_int64 was +2 B over the pin.
#[test]
fn a_chain_user_does_not_stop_a_truncated_load_narrowing() {
    for level in [Level::O1, Level::O2] {
        let text = assembly(IR, Width::W32, Options { level, ..Options::default() }).unwrap();
        assert!(text.contains("i32.load\ttable+8"), "{text}");
        assert!(!text.contains("i64.load") && !text.contains("i32.wrap_i64"), "{text}");
    }
}
