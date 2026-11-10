use std::path::Path;
use std::process::Command;

/// `while (!(x & mask)) x <<= 1` stayed a loop: `recognizeShiftUntilBitTest`
/// was not translated, so nothing made its trip count `ctlz` of the masked
/// value, and the loop kept shifting by one.
#[test]
fn a_loop_shifting_until_a_bit_is_set_is_a_ctlz() {
    for level in ["-O1", "-O2", "-Os"] {
        let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/shift_until_bit_test_loop.ll");
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
            .args(["-S", level, "-o", "-"])
            .arg(ir)
            .output()
            .expect("llrm-wasm-llc");
        assert!(out.status.success(), "{level}: {}", String::from_utf8_lossy(&out.stderr));
        let asm = String::from_utf8_lossy(&out.stdout).into_owned();
        assert_eq!(asm.matches("i32.clz").count(), 2, "{level}: want two i32.clz in:\n{asm}");
        assert!(!asm.contains("\tloop"), "{level}: a loop is left in:\n{asm}");
    }
}
