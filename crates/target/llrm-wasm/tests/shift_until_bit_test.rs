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

/// The loop's trip count mask `(1 << pos) | ((1 << pos) - 1)` stayed two
/// shifts and an add under an `or` (30 bytes for `until_bit`, the pin 23):
/// `visitOr` did not fold it to `-1 >>u (31 - pos)`.
#[test]
fn the_mask_through_the_tested_bit_is_one_shift_of_all_ones() {
    for level in ["-O1", "-O2", "-Os"] {
        let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/shift_until_bit_test_loop.ll");
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
            .args(["-S", level, "-o", "-"])
            .arg(ir)
            .output()
            .expect("llrm-wasm-llc");
        assert!(out.status.success(), "{level}: {}", String::from_utf8_lossy(&out.stderr));
        let asm = String::from_utf8_lossy(&out.stdout).into_owned();
        let variable_bit = asm.split(".functype").nth(1).unwrap_or_default();
        assert!(variable_bit.contains("i32.shr_u"), "{level}: want an i32.shr_u in:\n{asm}");
        assert!(!variable_bit.contains("i32.or"), "{level}: an i32.or is left in:\n{asm}");
    }
}
