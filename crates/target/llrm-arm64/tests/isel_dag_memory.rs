//! Loads, stores and shifted adds are the pin's instructions (`llc -O2`) for
//! the same IR.

use std::process::Command;

fn llc() -> (bool, String, String) {
    llc_of("dag_memory.ll")
}

fn llc_of(file: &str) -> (bool, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("t.s");
    let input = format!("{}/tests/data/{file}", env!("CARGO_MANIFEST_DIR"));
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "-o"])
        .arg(&output)
        .arg(input)
        .output()
        .unwrap();
    (
        said.status.success(),
        std::fs::read_to_string(&output).unwrap_or_default(),
        String::from_utf8_lossy(&said.stderr).into_owned(),
    )
}

/// Every function comes out of the table: before the load/store rows a `ldr`
/// was "has no arm64 instruction yet".
#[test]
fn every_function_is_selected_by_the_table() {
    let (ok, dag, why) = llc();
    assert!(ok, "{why}");
    let slot = &dag[dag.find("_ldx:").unwrap()..dag.find("_slot:").unwrap()];
    assert!(slot.contains("ldr x0, [x0, x1, lsl #3]"), "{slot}");
}

/// `x < 4097` is `x <= 4096` so the constant is the shifted immediate
/// `cmp w0, #1, lsl #12` (`getAArch64Cmp`); the hand selector materialized
/// 4097 with a `movz` and compared the registers.
#[test]
fn an_unencodable_compare_constant_moves_by_one_to_an_encodable_one() {
    let (ok, dag, why) = llc_of("dag_compare_immediate.ll");
    assert!(ok, "{why}");
    assert!(dag.contains("subs wzr, w0, #1, lsl #12"), "{dag}");
    assert!(dag.contains("csinc w0, wzr, wzr, gt"), "{dag}");
    assert!(!dag.contains("movz"), "{dag}");
}
