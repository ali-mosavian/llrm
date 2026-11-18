use std::path::Path;
use std::process::Command;

/// `tests/ir/redundant_copies.ll` as assembly at -O1, the mid-end skipped.
fn assembly() -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/redundant_copies.ll");
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O1", "-S", "-o", "-"])
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .arg(&ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    String::from_utf8_lossy(&made.stdout).into_owned()
}

/// Three copies of one register stayed three locals, `local.get 3; local.set N`
/// each (`PeepholeOptimizer::foldRedundantCopy`; shift_int32_variable +16 B
/// at -O1, where nothing coalesces them).
#[test]
fn copies_of_one_register_are_one_local() {
    let text = assembly();
    let sets = text.lines().filter(|line| line.contains("local.set") || line.contains("local.tee")).count();
    assert!(sets <= 2, "{text}");
}
