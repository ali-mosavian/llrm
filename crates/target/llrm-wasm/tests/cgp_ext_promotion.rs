use std::path::Path;
use std::process::Command;

/// CodeGenPrepare's `optimizeExt` did not run on wasm (only `SwitchType`
/// did), so a `sext` in the block after a loop stayed away from its byte
/// load: `i32.load8_u` and `i32.extend8_s` where the pin has one
/// `i32.load8_s` (stack_arrays_copy -O2, +17 B).
#[test]
fn a_sext_in_a_later_block_is_moved_to_its_load_and_folds_into_it() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/ext_load_across_loop.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout);
    assert!(asm.contains("i32.load8_s"), "{asm}");
    assert!(!asm.contains("i32.extend8_s"), "{asm}");
}
