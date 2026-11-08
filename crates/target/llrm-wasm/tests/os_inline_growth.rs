use std::path::Path;
use std::process::Command;

/// `tests/ir/os_inline_growth.ll`: a ten-operation helper called from four
/// callers, as assembly at `level`.
fn assembly(level: &str) -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/os_inline_growth.ll");
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args([level, "-S", "-o", "-"])
        .arg(&ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    String::from_utf8_lossy(&made.stdout).into_owned()
}

/// The pin's `OptSizeThreshold` inlined `cmp_bits` (101 B) into seven
/// callers at -Os: sfm16_* grew 549 B each. A size build keeps a callee that
/// would grow the code (`Tti::opt_size_inline_by_code_size`).
#[test]
fn a_size_build_keeps_a_helper_that_inlining_would_copy_into_every_caller() {
    let text = assembly("-Os");
    assert_eq!(text.matches("call\thelper").count() + text.matches("call helper").count(), 4, "{text}");
}

/// `terms` (a store-only callee whose pin cost is negative) stayed a call in
/// `line` at -Os: SZ1 held the inline back although the pin counts it as
/// shrinking the caller (frame_address_stack_args -Os, 290 B against the pin's
/// 252 B).
#[test]
fn a_size_build_inlines_a_callee_the_pin_prices_below_zero() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/os_inline_negative_cost.ll");
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-Os", "-S", "-o", "-"])
        .arg(&ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let text = String::from_utf8_lossy(&made.stdout);
    let line = text.split("\nline:").nth(1).expect("line");
    let line = line.split("end_function").next().unwrap();
    assert!(!line.contains("call\tterms") && !line.contains("call terms"), "{line}");
}
