//! A narrow constant in 32-bit code.

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
        .args(flags)
        .args(["-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// A short constant was `xor ax, ax` / `mov ax, 3`: an operand-size prefix
/// each (d_faces -Os: 57 of them) where the dword form reads the same.
#[test]
fn a_short_constant_is_written_whole() {
    let text = listing("short f(int c) { return c ? 3 : 0; }\n", &["-m32", "-mabi=sysv", "-O2", "-march=i486"]);
    assert!(text.contains("xor eax, eax") && text.contains("mov eax, 3"), "{text}");
    assert!(!text.contains(" ax, "), "{text}");
}

/// At -Os only zero is written whole: `mov eax, 3` is a byte longer than
/// `mov ax, 3`.
#[test]
fn a_short_constant_at_os_is_whole_only_when_zero() {
    let text = listing("short f(int c) { return c ? 3 : 0; }\n", &["-m32", "-mabi=sysv", "-Os", "-march=i486"]);
    assert!(text.contains("xor eax, eax") && text.contains("mov ax, 3"), "{text}");
}
