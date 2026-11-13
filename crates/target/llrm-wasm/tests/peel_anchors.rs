//! `peelLoop`'s anchor blocks (`.peel.begin`, `.peel.next`, `.peel.newph`)
//! count in the size of the loop around a peeled one (`LoopUnrollPass.cpp`'s
//! `LoopSize`).

use std::path::Path;
use std::process::Command;

/// The IR the backend gets for `tests/ir/peel_anchors.ll` at -O3.
fn ir() -> String {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/peel_anchors.ll");
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .env("LLRM_WASM_IR", "1")
        .args(["-O3", "-S", "-o", "/dev/null"])
        .arg(&source)
        .output()
        .expect("llrm-wasm-llc");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    String::from_utf8(made.stderr).expect("utf8")
}

/// Peeling the inner loop left no block of its own `br`, so the outer loop
/// measured 13, not the pin's 17: under wasm's partial threshold of 30 it was
/// unrolled by two with a remainder loop (known_loads_walks -O3: 292 B for this
/// reduction against the pin's 175).
#[test]
fn the_loop_around_a_peeled_one_is_sized_with_the_peel_anchors_and_is_not_unrolled() {
    let ir = ir();
    assert!(!ir.contains("xtraiter"), "{ir}");
}
