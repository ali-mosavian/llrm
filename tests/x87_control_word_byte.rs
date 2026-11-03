//! The truncating control word is built with a byte `or`.

mod common;

/// `or word [chop], 0x0C00` carried a 2-byte immediate (and a 66h prefix at
/// m32): 6 B at m32 and 5 B at m16 where `or byte [chop+1], 0x0C` is 4.
#[test]
fn test_the_truncating_control_word_is_set_with_a_byte_or() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), "int f(long double x) { return (int)x; }\n").unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["--target=x86_32-wat", "-m32", "-mabi=sysv", "-mlong-double-80", "-march=i486", "-Os", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let or = text.lines().find(|line| line.trim_start().starts_with("or ")).unwrap_or_else(|| panic!("{text}"));
    assert!(or.contains("byte") && or.contains("12"), "{or}\n{text}");
}
