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

/// A two-register phi (`i128`/`fp128`) copied only its first register on the back edge, so the
/// second half stayed stale: `long_double_variadic -O2` returned 0 where pin clang returns 13.
#[test]
fn both_registers_of_a_wide_phi_are_copied_on_the_back_edge() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/i128_phi_swap.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O0", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    // registers 7..=10 are the four halves of the two phis: set at entry and again on the back edge
    for r in 7..=10 {
        let n = asm.matches(&format!("local.set\t{r}\n")).count();
        assert!(n >= 2, "register {r} set {n} time(s) in:\n{asm}");
    }
}

/// The zero an `fp128` phi starts from was bitcast to `i128` and expanded through a stack slot
/// (`ExpandRes_BITCAST` skipped `TypeSoftenFloat`), leaving a dead 16-byte frame object per use: the
/// frame was 48 bytes where the pin's is 16 (`long_double_variadic` -Os wasm32, +33 B).
#[test]
fn the_zero_an_fp128_phi_starts_from_takes_no_frame_object() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/fp128_phi.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(asm.contains("global.get\t__stack_pointer\n\ti32.const\t16\n\ti32.sub"), "frame is not 16 bytes in:\n{asm}");
}
