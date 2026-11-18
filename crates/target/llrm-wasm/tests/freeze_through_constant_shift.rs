use std::path::Path;
use std::process::Command;

/// The pin pushes a freeze through `shl` by a constant in range
/// (`canCreateUndefOrPoison` of a shift), so `sra (freeze (shl x, 24)), 24` is
/// still `sext_inreg`. Ours left `shl` out of the push (DEVIATE SD213): the
/// freeze sat between the shifts and div8_os -O1 kept `i32.const 24; i32.shl;
/// i32.const 24; i32.shr_s` for `i32.extend8_s` (+10 B).
#[test]
fn a_frozen_shift_pair_is_a_sign_extension() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/freeze_through_constant_shift.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(asm.contains("i32.extend8_s"), "{asm}");
    assert!(!asm.contains("i32.shl"), "{asm}");
}
