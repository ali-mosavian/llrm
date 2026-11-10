use std::path::Path;
use std::process::Command;

/// `optimizeBranch` runs on wasm and does nothing: its hook,
/// `preferZeroCompareBranch`, is the base class's `false` (x86 alone says
/// yes), so the compare with a constant stays. A premise test: no fail-first,
/// the step has no effect here; it fails if the hook ever answers yes.
#[test]
fn a_compare_with_a_constant_is_not_turned_into_a_compare_of_the_shift() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/branch_on_shift_of_compared.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .env("LLRM_WASM_IR", "1")
        .args(["-O2", "-filetype=obj", "-o", "/dev/null"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let backend_ir = String::from_utf8_lossy(&out.stderr);
    assert!(backend_ir.contains("icmp ult i32 %x, 8"), "{backend_ir}");
}
