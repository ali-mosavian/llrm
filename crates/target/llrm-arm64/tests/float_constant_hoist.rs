//! A float constant made in a loop (`mov w, #lo; movk w, #hi; fmov s, w`) had
//! only its `mov`/`movk` hoisted; the `fmov` stayed, so the held GPR constant
//! was split and made again on both loop latches: jpeg_single -O1 with the
//! vectors on made 0x3fb374bc and 0xbf36d19e 3 times each where the pin makes
//! them once (+12 B against the vectors off). The pin's `MachineLICM` hoists
//! the cross-class `COPY` with its source.

use std::process::Command;

#[test]
fn a_float_constant_made_in_loops_is_made_once() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("t.s");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .env("LLRM_ARM64_VECTORS", "1")
        .args(["-S", "--no-opt", "-O1", "-o"])
        .arg(&output)
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/jpeg_single_o1.ll"))
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let text = std::fs::read_to_string(&output).unwrap();
    // The high halves of 0x3fb374bc (#16307) and 0xbf36d19e (#48950).
    for high in ["#16307", "#48950"] {
        let made = text.lines().filter(|line| line.contains("movk") && line.contains(high)).count();
        assert_eq!(made, 1, "{high} made {made} times\n{text}");
    }
}
