//! `atomicrmw xchg` and a `seq_cst` store went to the old selector: the DAG
//! had no atomic node (SD253). They are `ATOMIC_SWAP` now, an `xchg` each.

#[test]
fn a_function_with_an_atomic_exchange_is_selected_by_the_dag() {
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/check/mir/atomic_xchg.ll");
    let scratch = tempfile::tempdir().unwrap();
    let report = scratch.path().join("who");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .env("LLRM_ISEL_REPORT", &report)
        .args(["-O2", "-S", "-o", "-", fixture])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text.matches("xchg ").count(), 2, "{text}");
    let who = std::fs::read_to_string(&report).unwrap();
    assert!(who.lines().any(|line| line.ends_with(" dag") && line.contains("take")), "selected by: {who}");
}
