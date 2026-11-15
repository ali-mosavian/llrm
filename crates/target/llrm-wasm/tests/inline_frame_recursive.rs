use std::path::Path;
use std::process::Command;

/// A callee with a 64-byte frame was never copied into a recursive caller
/// (`inline_frames_into_recursive_callers` said no), where the pin allows up to
/// 1024 bytes (`TotalAllocaSizeRecursiveCaller`): `f` stayed a function called
/// from `g` (shrink_wrapped_frame -O1: 395 B and 2,901 instrs against the pin's
/// 386 B and 2,868).
#[test]
fn a_small_frame_is_copied_into_a_recursive_caller() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/inline_frame_recursive.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O1", "-S", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!text.lines().any(|l| l.trim_start().starts_with("f:")), "f is still a function:\n{text}");
}
