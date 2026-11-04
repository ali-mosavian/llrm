use std::path::Path;
use std::process::Command;

/// A phi's register carried nothing (`ComputePHILiveOutRegInfo` was missing),
/// so the join of `true` and a compare was not known to be 0 or 1: its
/// negation was `x ^ -1` masked by `& 1` where the pin knows `x ^ 1`.
/// jpeg_single -O1 paid 12 B for it.
#[test]
fn a_phi_of_booleans_is_known_to_be_zero_or_one() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/phi_live_out.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout);
    assert!(!asm.contains("i32.const\t-1"), "{asm}");
}
