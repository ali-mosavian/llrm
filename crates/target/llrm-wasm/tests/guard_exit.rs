use std::path::Path;
use std::process::Command;

/// The block of a loop guard's exit held only a `br` once the folder hoisted
/// its `i32.const 0` into the guard, and was not forwarded: the function
/// returned twice, `local.get; return` before the loop and `local.get; end`
/// after it (3 B over the pin's 243: recgcd, switch, recpow at -O2).
#[test]
fn a_block_the_hoisting_leaves_with_only_a_branch_is_forwarded() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/guard_exit.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O2", "-S", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout);
    assert!(!asm.lines().any(|line| line.trim() == "return"), "{asm}");
}
