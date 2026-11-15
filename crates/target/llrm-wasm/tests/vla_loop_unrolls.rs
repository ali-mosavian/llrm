use std::path::Path;
use std::process::Command;

/// vla_callee_constant wasm -O2 ran 2,027 instructions over the pin: the
/// `stacksave`/`stackrestore` in a loop had no price, so the six-trip loop
/// "could not be unrolled" and `nested` was never folded. The pin prices them
/// as a library call (1 for code size) and unrolls.
#[test]
fn a_loop_with_a_stack_save_and_restore_is_fully_unrolled() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/vla_loop_unrolls.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let asm = String::from_utf8_lossy(&out.stdout);
    assert!(!asm.contains("loop"), "{asm}");
}
