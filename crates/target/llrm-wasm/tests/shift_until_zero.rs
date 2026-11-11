use std::path::Path;
use std::process::Command;

/// `while (x >>= 1) n++` stayed a loop: bitcount_idioms `log2_32` was 40 bytes
/// at -O1 against the pin's 12, because `recognizeAndInsertFFS` was not
/// translated. Both shift directions, both ways to keep `ctlz` defined, and the
/// `u<` forms (`recognizeShiftUntilLessThan`).
#[test]
fn a_loop_shifting_to_zero_is_a_clz() {
    for level in ["-O1", "-O2", "-Os"] {
        let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/shift_until_zero_loop.ll");
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
            .args(["-S", level, "-o", "-"])
            .arg(ir)
            .output()
            .expect("llrm-wasm-llc");
        assert!(out.status.success(), "{level}: {}", String::from_utf8_lossy(&out.stderr));
        let asm = String::from_utf8_lossy(&out.stdout).into_owned();
        // The right shifts count leading zeros, `shl_count` trailing ones.
        assert_eq!(asm.matches("i32.clz").count(), 4, "{level}: want four i32.clz in:\n{asm}");
        assert_eq!(asm.matches("i32.ctz").count(), 1, "{level}: want one i32.ctz in:\n{asm}");
        assert!(!asm.contains("\tloop"), "{level}: a loop is left in:\n{asm}");
    }
}
