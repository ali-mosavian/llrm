use std::path::Path;
use std::process::Command;

/// `tests/ir/setcc_logic.ll` as assembly at -O2.
fn assembly() -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/setcc_logic.ll");
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O2", "-S", "-o", "-"])
        .arg(&ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    String::from_utf8_lossy(&made.stdout).into_owned()
}

/// `(a != 7) | (b != 9)` was folded to `((a ^ 7) | (b ^ 9)) != 0` as on x86
/// (`convertSetCCLogicToBitwiseLogic`); WebAssembly does not, and the pin keeps
/// two `ne` and an `or` (memcpy_libcall +2 B at -O2, -O3).
#[test]
fn an_or_of_two_not_equal_compares_is_not_folded_into_xors() {
    let text = assembly();
    assert!(!text.contains("i32.xor"), "{text}");
    assert_eq!(text.matches("i32.ne").count(), 2, "{text}");
}
