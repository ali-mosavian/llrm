//! A tied quadword `shl` by 1 to 3 whose source lives on is a `lea`
//! (`X86InstrInfo::convertToThreeAddress`, SHL64ri to LEA64r).

use std::process::Command;

/// `x << 2` with `x` read again: the pin writes `lea rax, [4*rdi]`. llrm made
/// the three-address form for dwords only, so the quadword was `mov rax, rdi;
/// shl rax, 2` (and `x * 4`, `x * 8`, `x * 7` the same).
#[test]
fn test_a_quadword_shift_by_two_whose_source_lives_on_is_a_lea() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("a.c");
    std::fs::write(&source, "long f(long x) { return (x << 2) ^ x; }\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["-m64", "-O2", "-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(text.lines().any(|line| line.trim().starts_with("lea rax, [rdi*4]")), "{text}");
    assert!(!text.lines().any(|line| line.trim().starts_with("shl ")), "{text}");
}

/// `x + y` with both read again: the pin writes `lea rax, [rsi + rdi]`
/// (ADD64rr to LEA64r); llrm copied one operand and added.
#[test]
fn test_a_quadword_add_whose_sources_live_on_is_a_lea() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("a.c");
    std::fs::write(&source, "long f(long x, long y) { return (x + y) ^ x ^ y; }\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["-m64", "-O2", "-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(text.lines().any(|line| line.trim().starts_with("lea ") && line.contains("rsi+rdi")), "{text}");
}
