use std::path::Path;
use std::process::Command;

fn asm(level: &str) -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/popcount_loop.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", level, "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{level}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// `for (; x; x &= x - 1) n++` stayed a loop: bitcount_idioms `ones32` was 59
/// bytes at -O1 against the pin's 6, `ones64` 66 against 7, because
/// `recognizePopcount` was not translated.
#[test]
fn a_loop_clearing_the_lowest_bit_is_a_popcnt() {
    for level in ["-O1", "-O2", "-Os"] {
        let asm = asm(level);
        assert!(asm.contains("i32.popcnt"), "{level}: no i32.popcnt in:\n{asm}");
        assert!(asm.contains("i64.popcnt"), "{level}: no i64.popcnt in:\n{asm}");
        assert!(!asm.contains("\tloop"), "{level}: a loop is left in:\n{asm}");
    }
}
