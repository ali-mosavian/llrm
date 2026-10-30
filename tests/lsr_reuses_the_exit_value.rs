//! A loop's exit test reuses the narrow limit the exit value already holds.

use std::process::Command;

const PROGRAM: &str = "dim stopAt as integer
dim counter as integer
dim total as long
data 5
read stopAt
counter = 0
total = 0
do while counter < 10
    if counter = stopAt then exit do
    total = total + counter
    counter = counter + 1
loop
print counter
print total
end
";

/// lcexit.bas m16 -Os grew 30 bytes: LSR expanded the 16-bit limit again as a
/// 32-bit `umin` and truncated it, beside the 16-bit `umin` of the exit value.
#[test]
fn at_os_the_exit_test_does_not_expand_the_limit_a_second_time() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.bas"), PROGRAM).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .args(["-Os", "-m16", "-march=i386", "-S", "-o", "a.s", "a.bas"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert_eq!(text.matches(", 9\n").count(), 2, "{text}");
}
