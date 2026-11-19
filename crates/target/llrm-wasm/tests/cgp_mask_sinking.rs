use std::path::Path;
use std::process::Command;

/// `sinkAndCmp0Expression` runs on wasm and does nothing: its hook,
/// `isMaskAndCmp0FoldingBeneficial`, is the base class's `false`, so an `and`
/// compared with zero in two blocks stays one `and`. A premise test: no
/// fail-first, the step has no effect here; it fails if the hook ever says yes.
#[test]
fn an_and_compared_with_zero_in_two_blocks_is_not_made_again() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/mask_compared_in_two_blocks.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .env("LLRM_WASM_IR", "1")
        .args(["-O2", "-filetype=obj", "-o", "/dev/null"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let backend_ir = String::from_utf8_lossy(&out.stderr);
    assert_eq!(backend_ir.matches(" = and i32 ").count(), 1, "{backend_ir}");
}
