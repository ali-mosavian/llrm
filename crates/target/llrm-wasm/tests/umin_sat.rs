use std::path::Path;
use std::process::Command;

/// `umin (smax x, 0), 255` has operands of one sign, so the pin's
/// `visitIMINMAX` flips it to `smin` whether or not `umin` is legal
/// (`IsSatBroken`). Ours kept `umin` and compared `lt_u` where the pin has
/// `lt_s` (jpeg_fixed -O2, three clamps per pixel).
#[test]
fn a_clamp_of_a_non_negative_value_compares_signed() {
    let asm = asm("umin_sat.ll");
    assert!(asm.contains("i32.lt_s") && !asm.contains("i32.lt_u"), "{asm}");
}

/// The same `umin` of a value whose sign is unknown stays unsigned.
#[test]
fn an_unsigned_min_of_an_unknown_sign_stays_unsigned() {
    let asm = asm("umin_mixed.ll");
    assert!(asm.contains("i32.lt_u") && !asm.contains("i32.lt_s"), "{asm}");
}

fn asm(file: &str) -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir").join(file);
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout).into_owned()
}
