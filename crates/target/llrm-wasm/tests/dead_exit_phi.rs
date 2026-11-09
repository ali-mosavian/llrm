use std::path::Path;
use std::process::Command;

/// An exit PHI nothing reads (its only use, a store to a dead slot, was
/// removed) kept a `local.get; drop` in `walk`: ours never deleted a dead PHI,
/// the pin does. long_double_variadic -O1: +9 B of its +24.
#[test]
fn a_dead_exit_phi_leaves_no_drop() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/dead_exit_phi.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    let from = asm.find("\nwalk:").expect("walk");
    let walk = asm[from..].split("end_function").next().unwrap();
    assert!(!walk.contains("drop"), "{walk}");
}
