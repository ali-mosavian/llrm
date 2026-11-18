use std::path::Path;
use std::process::Command;

/// `tests/ir/select_sequence.ll` as assembly at -O2.
fn assembly() -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/select_sequence.ll");
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O2", "-S", "-o", "-"])
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .arg(&ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    String::from_utf8_lossy(&made.stdout).into_owned()
}

/// `select (a != 0 && b != 0), 5, 0` is two selects, as the pin's
/// `shouldNormalizeToSelectSequence` is true for wasm; merged into an `and` of
/// the compares it cost a `local.get`, a `const` and an `ne` more
/// (compare_to_int64 +1 B at -O1, -Os).
#[test]
fn a_select_on_an_and_of_compares_is_a_sequence_of_selects() {
    let text = assembly();
    assert!(!text.contains("i32.and"), "{text}");
    assert_eq!(text.matches("select").count(), 2, "{text}");
}
