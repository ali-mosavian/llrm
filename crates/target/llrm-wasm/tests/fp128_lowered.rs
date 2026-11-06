use std::path::Path;
use std::process::Command;

/// An `fp128` function was refused ("fp128: no legalization yet"). It is softened to the `i128` of its
/// bits: two `i64` per operand, a result through a leading pointer, and the compiler-rt tf routine
/// the pin names for each operation (`llc -mtriple=wasm32 -O2` of the same module gave these lines).
#[test]
fn an_fp128_operation_is_a_call_to_the_tf_routine_with_the_i128_abi() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/fp128_ops.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    for want in [
        ".functype\tadd (i32, i64, i64, i64, i64) -> ()",
        ".functype\tfromi (i32, i32) -> ()",
        ".functype\ttoi (i64, i64) -> (i64)",
        ".functype\tst (i32, i64, i64) -> ()",
        ".functype\tld (i32, i32) -> ()",
        "call\t__addtf3",
        "call\t__subtf3",
        "call\t__multf3",
        "call\t__floatsitf",
        "call\t__fixtfdi",
        "call\t__fixunstfdi",
    ] {
        assert!(asm.contains(want), "no `{want}` in:\n{asm}");
    }
}

/// A phi of an `fp128` (or an `i128`) at `-O0` was refused ("a wasm signature with Float(F128)"):
/// it is two `i64` registers, copied as two.
#[test]
fn an_fp128_phi_is_two_i64_registers() {
    for level in ["-O0", "-O2"] {
        let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/fp128_phi.ll");
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
            .args(["-S", level, "-o", "-"])
            .arg(ir)
            .output()
            .expect("llrm-wasm-llc");
        assert!(out.status.success(), "{level}: {}", String::from_utf8_lossy(&out.stderr));
        assert!(String::from_utf8_lossy(&out.stdout).contains("call\t__addtf3"), "{level}");
    }
}
