//! gcc's bb-reorder `copy_bb_p`: a branch to a block that only returns gets its own copy of it.

use std::process::Command;

const KERNEL: &str = "
static int hanoi(int n, int a, int b, int c)
{
    if (n == 0) return 0;
    return hanoi(n - 1, a, c, b) + 1 + hanoi(n - 1, c, b, a);
}
int f(int n) { return hanoi(n, 1, 3, 2); }
";

fn listing(flags: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), KERNEL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(flags)
        .args(["-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// The early exit of the recursion jumped to the one return the loop's end also reached, so the two could not be told
/// apart (no shrink-wrapping): one `ret` in hanoi, with a `jmp` to it from the entry test.
#[test]
fn test_the_early_exit_of_a_recursion_returns_in_a_block_of_its_own() {
    let text = listing(&["-m32", "-mabi=sysv", "-march=i486", "-O2", "-fno-inline-functions"]);
    let body = text
        .split("_hanoi proc")
        .nth(1)
        .and_then(|rest| rest.split("endp").next())
        .unwrap_or_else(|| panic!("no hanoi: {text}"));
    let returns = body.lines().filter(|line| line.trim() == "ret").count();
    assert!(returns >= 2, "{returns} return: {body}");
}

/// -Os keeps one return: the copy is for speed.
#[test]
fn test_size_keeps_the_shared_return() {
    let text = listing(&["-m32", "-mabi=sysv", "-march=i486", "-Os", "-fno-inline-functions"]);
    let body = text
        .split("_hanoi proc")
        .nth(1)
        .and_then(|rest| rest.split("endp").next())
        .unwrap_or_else(|| panic!("no hanoi: {text}"));
    assert_eq!(body.lines().filter(|line| line.trim() == "ret").count(), 1, "{body}");
}
