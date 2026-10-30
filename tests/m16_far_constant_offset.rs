//! A far byte read at a constant offset past 32767 compiles.

use std::process::Command;

/// `DEF SEG = 0: PEEK(&HB8000 + 3998)` failed to compile under the DAG
/// selector (offset sign-extended to -28770; 9 QB programs: pdhuge, hugerg,
/// defseg_constant.flat ...). The old selector writes `es:[36766]`.
#[test]
fn test_a_far_read_at_a_high_constant_offset_compiles() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.bas"), "DEF SEG = 0\nPRINT PEEK(&HB8000 + 3998)\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .args(["--isel=dag", "a.bas", "-m16", "-S", "-o", "-"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("es:[36766]"));
}
