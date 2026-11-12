use std::path::Path;
use std::process::Command;

/// `tests/ir/dead_phi_cycle.ll` as assembly at -O2, the mid-end skipped: it deletes the cycle.
fn assembly() -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/dead_phi_cycle.ll");
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O2", "-S", "-o", "-"])
        .arg(&ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    String::from_utf8_lossy(&made.stdout).into_owned()
}

/// Two phis that only feed each other joined the live pair's registers, and the copies they left
/// (`%d = COPY %0` twice) stayed: an `i32.const 0; drop` per phi (int64_divmod_edges +24 B at -O2).
/// The coalescer erases a copy that makes the register hold what it holds; the function must too.
#[test]
fn a_phi_cycle_nothing_reads_leaves_no_dropped_constant() {
    let text = assembly();
    assert!(!text.contains("drop"), "{text}");
}
