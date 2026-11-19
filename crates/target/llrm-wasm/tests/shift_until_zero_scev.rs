use std::path::Path;
use std::process::Command;

/// `for (iv = start; (val >> (iv + off)) != 0; iv++);` stayed a loop:
/// `recognizeShiftUntilZero` was not translated, so the trip count was never
/// `ctlz`/`cttz` of the value.
#[test]
fn a_loop_shifting_a_value_by_a_growing_amount_is_a_count() {
    for level in ["-O1", "-O2", "-Os"] {
        let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/shift_until_zero_scev_loop.ll");
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
            .args(["-S", level, "-o", "-"])
            .arg(ir)
            .output()
            .expect("llrm-wasm-llc");
        assert!(out.status.success(), "{level}: {}", String::from_utf8_lossy(&out.stderr));
        let asm = String::from_utf8_lossy(&out.stdout).into_owned();
        assert_eq!(asm.matches("i32.clz").count(), 1, "{level}: want one i32.clz in:\n{asm}");
        assert_eq!(asm.matches("i32.ctz").count(), 1, "{level}: want one i32.ctz in:\n{asm}");
        assert!(!asm.contains("\tloop"), "{level}: a loop is left in:\n{asm}");
    }
}
