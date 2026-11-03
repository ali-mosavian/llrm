use std::path::Path;
use std::process::Command;

/// A division cost 1 in the unroll estimate, where the base
/// `getArithmeticInstrCost` answers `TCC_Expensive` for every cost kind but
/// throughput: nbody_fixed's 6-trip inner loop was unrolled (12 `div_s` against
/// the pin's 2), 1311 bytes against the pin's 993.
#[test]
fn a_loop_of_divisions_is_not_unrolled_for_a_cheap_estimate() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/div_cost.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O2", "-S", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout);
    assert_eq!(asm.matches("div_s").count(), 2, "{asm}");
}
