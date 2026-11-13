use std::path::Path;
use std::process::Command;

/// `tests/ir/and_of_truncated_carry.ll` through the backend alone, as assembly.
fn assembly() -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/and_of_truncated_carry.ll");
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .args(["-O3", "-S", "-o", "-"])
        .arg(&ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    String::from_utf8(made.stdout).expect("utf8")
}

/// `and i8 (a carry-checked sum), -8` widened to `and x, 504` (the sum's nine
/// bits sign-extended), not the pin's `and x, 248` (`aext (and (trunc x), c)`
/// is `and x, zext c`): a mask the loop then re-applied on every trip, +4 B in
/// `known_loads_walks` at -O3.
#[test]
fn an_and_of_a_truncated_value_keeps_the_constant_zero_extended() {
    let asm = assembly();
    assert!(asm.contains("i32.const\t248"), "{asm}");
    assert!(!asm.contains("i32.const\t504"), "{asm}");
}
