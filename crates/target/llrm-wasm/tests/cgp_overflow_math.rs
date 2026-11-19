use std::path::Path;
use std::process::Command;

/// CodeGenPrepare's `combineToUAddWithOverflow` did not run on wasm: `icmp
/// ult (add a, b), a` with the sum also read kept the add and the compare
/// apart. The pin forms `llvm.uadd.with.overflow`, as the backend's input
/// shows.
#[test]
fn a_wrapped_sum_check_with_the_sum_used_becomes_an_overflow_add() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/sum_compared_with_operand.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .env("LLRM_WASM_IR", "1")
        .args(["-O2", "-filetype=obj", "-o", "/dev/null"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let backend_ir = String::from_utf8_lossy(&out.stderr);
    assert!(backend_ir.contains("@llvm.uadd.with.overflow.i32("), "{backend_ir}");
}
