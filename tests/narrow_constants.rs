//! A narrow constant in 32-bit code.

mod common;

use std::process::Command;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output =
        common::cc::llrm_c().current_dir(directory).args(flags).args(["-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// A short zero was `xor ax, ax`: an operand-size prefix (d_faces -Os: 27 of
/// them) where `xor eax, eax` reads the same. Another constant stays a word:
/// `mov eax, 3` is a byte longer, and widening it lost shellsort a folded load.
#[test]
fn a_short_zero_is_written_whole() {
    for level in ["-O2", "-Os"] {
        let text = listing("short f(int c) { return c ? 3 : 0; }\n", &["-m32", "-mabi=sysv", level, "-march=i486"]);
        assert!(text.contains("xor eax, eax") && text.contains("mov ax, 3"), "{level}: {text}");
    }
}
