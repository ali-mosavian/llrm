//! `llrm-cc` forwards `--isel=` to every target's llc. The arm64 llc has one
//! selector, so `--isel=dag` is accepted and any other value is a named error.

use std::process::{Command, Output};

fn llc(isel: &str) -> (Output, std::path::PathBuf, tempfile::TempDir) {
    let scratch = tempfile::tempdir().unwrap();
    let out = scratch.path().join("t.s");
    let input = format!("{}/tests/data/dag_memory.ll", env!("CARGO_MANIFEST_DIR"));
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", isel, "-o"])
        .arg(&out)
        .arg(input)
        .output()
        .unwrap();
    (said, out, scratch)
}

/// After the hand selector was deleted `llrm-cc --isel=dag` on arm64 exited 2
/// ("unrecognized argument --isel=dag") where it had compiled.
#[test]
fn isel_dag_is_accepted() {
    let (said, out, _scratch) = llc("--isel=dag");
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    assert!(out.exists());
}

#[test]
fn another_selector_is_refused_by_name() {
    for mode in ["--isel=hand", "--isel=shadow"] {
        let (said, out, _scratch) = llc(mode);
        let stderr = String::from_utf8_lossy(&said.stderr);
        assert_eq!(said.status.code(), Some(2), "{mode}: {stderr}");
        assert!(stderr.contains(mode) && stderr.contains("only selector"), "{mode}: {stderr}");
        assert!(!out.exists());
    }
}
