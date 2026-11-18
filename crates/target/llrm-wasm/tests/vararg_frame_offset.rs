use std::path::Path;
use std::process::Command;

/// The frame object a variadic call makes is as aligned as any (the pin's
/// `computeKnownBitsForFrameIndex`), so `add FI, 8` is `or disjoint` and folds
/// into the store's offset. The DAG knew only the IR's allocas: the store kept
/// `i32.const 8; i32.add` (variadic -O1, +4 B).
#[test]
fn a_variadic_call_buffer_store_folds_its_offset() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/vararg_frame_offset.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O1", "-S", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("i64.store\t8") || text.contains("i64.store 8") || text.contains("i64.store\t8:p2align=3"),
        "{text}"
    );
    assert!(!text.contains("i32.add\n\ti64.const"), "{text}");
}
