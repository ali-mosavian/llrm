//! Zeroed locals that follow each other share one fill.

use std::process::Command;

/// matmul.nib zeroes `a`, `b` and `c`: three `lea di; mov cx; rep stosw`
/// fills, 314 B at -Os against bcc's 298. One fill of the three is 300.
#[test]
fn test_three_zeroed_arrays_are_cleared_by_one_fill() {
    let source = format!("{}/bench/matmul/matmul.nib", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
        .current_dir(scratch.path())
        .args(["-Os", "-m16", "-march=i486", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let fills = text.lines().filter(|line| line.trim() == "rep stosw").count();
    assert_eq!(fills, 1, "{text}");
}
