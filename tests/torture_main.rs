//! Failures of gcc.c-torture/execute that a source program shows: each a C
//! program, its listing read.

use std::process::Command;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(["-m32", "-march=i486"])
        .args(flags)
        .args(["-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// `int main(void)` that reaches its closing brace returns 0 (C99 5.1.2.2.3):
/// alias-access-path-1 and kin exited with whatever the last compare left in
/// eax (2), at -O2 and -Os, because a function with no return value was made
/// void.
#[test]
fn main_that_falls_off_its_end_returns_zero() {
    for level in ["-O0", "-O2", "-Os"] {
        let text = listing(
            "extern void abort(void);\nint v;\nint main(void)\n{\n    v = 2;\n    if (v == 3) abort();\n}\n",
            &[level],
        );
        let body = text.split("_main proc").nth(1).and_then(|rest| rest.split("endp").next()).unwrap_or_default();
        assert!(["xor eax, eax", "mov eax, 0", "mov al, 0"].iter().any(|zero| body.contains(zero)), "{level}: {body}");
    }
}
