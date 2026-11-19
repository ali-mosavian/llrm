use std::path::Path;
use std::process::Command;

/// The late unroll took the upper-bound count of an inner loop of at most three
/// trips, then asked for a runtime remainder it could not make, and left the
/// loop rolled (two `loop`s; the pin unrolls it whole: `Runtime = false` at the
/// max trip count).
#[test]
fn a_loop_of_at_most_three_trips_unrolls_by_its_upper_bound() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/unroll_upper_bound.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    let from = asm.find("\npass_gap:").expect("pass_gap");
    let body = asm[from..].split("end_function").next().unwrap();
    let loops = body.lines().filter(|line| line.trim() == "loop").count();
    assert_eq!(loops, 1, "{body}");
}
