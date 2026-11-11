use std::path::Path;
use std::process::Command;

/// jpeg_fixed wasm -Os ran 132 B over the pin: `put32`'s four-trip byte-store
/// loop stayed a loop. A `trunc` from `i32` to `i8` was priced 1, not the
/// pin's 0 (both legalize to `i32`), so the loop's size 8 was one over what
/// unrolling it costs, and the pin's unrolled copy never reached the callers.
#[test]
fn a_four_trip_byte_store_loop_is_unrolled_at_os() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/put32_loop.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-Os", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let asm = String::from_utf8_lossy(&out.stdout);
    assert!(!asm.contains("loop"), "{asm}");
}
