use std::path::Path;
use std::process::Command;

/// CodeGenPrepare's `SinkCast` did not run on wasm: a `trunc i32 to i8`
/// read in two blocks stayed in the first, its i8 and i32 forms both live
/// across the edge (bench/jpeg_fixed -Os, +4 B). The pin makes the cast again
/// in the block that reads it.
#[test]
fn a_noop_trunc_read_in_another_block_is_made_again_there() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/trunc_read_in_two_blocks.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .env("LLRM_WASM_IR", "1")
        .args(["-O2", "-filetype=obj", "-o", "/dev/null"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let backend_ir = String::from_utf8_lossy(&out.stderr);
    assert_eq!(backend_ir.matches("trunc i32 %x to i8").count(), 2, "{backend_ir}");
}
