use std::path::Path;
use std::process::Command;

/// `tests/ir/tail_call_return.ll` as assembly at -O1.
fn assembly() -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/tail_call_return.ll");
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O1", "-S", "-o", "-"])
        .arg(&ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    String::from_utf8_lossy(&made.stdout).into_owned()
}

/// CodeGenPrepare copied the `ret` into the block of a `tail` call, although
/// WebAssembly's `mayBeEmittedAsTailCall` is the base class's `false`. The
/// two returns changed the block order: `i32.eqz; br_if` where the pin has a
/// bare `br_if` (tail_calls -O1 +1 B).
#[test]
fn a_return_after_a_tail_call_is_not_copied_into_the_calling_block() {
    let text = assembly();
    assert!(!text.contains("i32.eqz"), "{text}");
}
