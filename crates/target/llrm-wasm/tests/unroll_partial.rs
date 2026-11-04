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
    // The loop keeps its back edge and tests the exit once per four copies; a
    // complete unroll (no back edge) mis-summed every n but 4
    // (object_extent_loop).
    let text = String::from_utf8_lossy(&out.stdout);
    let body = text.split("\tloop").nth(1).and_then(|rest| rest.split("end_loop").next()).expect("a loop");
    assert_eq!((body.matches("f64.mul").count(), body.matches("br_if").count()), (4, 1), "{body}");
}

/// The epilog loop of a run-time unroll was unrolled again by two (the pin
/// marks it `setLoopAlreadyUnrolled`): `object_extent_loop` at wasm O2 was
/// 1680 B where the pin's is 1489.
#[test]
fn the_remainder_of_a_run_time_unroll_is_not_unrolled_again() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/unroll_runtime_nested.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let muls = String::from_utf8_lossy(&out.stdout).matches("f64.mul").count();
    assert_eq!(muls, 5, "{muls} multiplies: four in the body, one in the remainder");
}
