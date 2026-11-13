use std::path::Path;
use std::process::Command;

/// A variadic call's buffer is `getAlignedCallFrameSize` bytes (the stack size
/// aligned to the widest argument), at the stack alignment. It was rounded up
/// to the stack alignment too: one `i32` took 16 bytes, so a 24-byte local
/// before it made a 48-byte frame where the pin's is 32 (variadic -O1, +3 B).
#[test]
fn a_variadic_buffer_is_as_large_as_its_arguments_not_the_stack_alignment() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/vararg_buffer_size.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O2", "-S", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("i32.const\t32\n\ti32.sub"), "{text}");
}
