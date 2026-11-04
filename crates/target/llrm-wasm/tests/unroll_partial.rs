use std::path::Path;
use std::process::Command;

/// `LoopUnrollPass` (wasm: partial threshold 30) unrolls the 40-trip inner
/// loop of bench/tile five times. The full unroller left it rolled: `tile`
/// ran 94725 instructions to the pin's 57229.
#[test]
fn a_loop_of_forty_trips_and_six_instructions_is_unrolled_by_five() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/unroll_partial.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let loads = String::from_utf8_lossy(&out.stdout).matches("i32.load16_s").count();
    assert_eq!(loads, 5, "{loads} loads in the inner loop");
}

/// A dot loop with a run-time trip count was left rolled (`runtime unroll is
/// not translated`); wasm's `dot` ran one multiply per trip where the pin
/// runs four, with a remainder loop.
#[test]
fn a_loop_of_unknown_trips_is_unrolled_by_four_with_a_remainder() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/unroll_runtime.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let muls = String::from_utf8_lossy(&out.stdout).matches("f64.mul").count();
    assert_eq!(muls, 5, "{muls} multiplies: four in the body, one in the remainder");
}
