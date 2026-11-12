use std::path::Path;
use std::process::Command;

/// `tests/ir/preferred_extend.ll` as assembly at -O2.
fn assembly() -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/preferred_extend.ll");
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O2", "-S", "-o", "-"])
        .arg(&ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    String::from_utf8_lossy(&made.stdout).into_owned()
}

/// A value read signed in another block is exported sign-extended (`PreferredExtendType`): the `i16`
/// compared with `slt` in the loop loads with `load16_s` before it. Exported any-extended it was
/// `load16_u` and an `extend16_s` in the loop (phi_slot_unknown_arm +1 B at -O2, -O3, -Os).
#[test]
fn a_value_compared_signed_in_another_block_is_loaded_sign_extended() {
    let text = assembly();
    assert!(!text.contains("extend16_s"), "{text}");
}
