//! `ctlz` and `cttz` of a word at flat 32-bit x86 are `bsr` and `bsf`.

use std::process::Command;

/// `__builtin_clz` and `__builtin_ctz` went through the shift-and-popcount
/// expansion (`expand = [.. "ctlz", "cttz" ..]` of the m32 description, 25 or
/// more instructions each, `and eax, 1431655765` among them). The pin's llc
/// at i486 writes `bsr; xor 31` and `bsf`.
#[test]
fn test_a_word_count_of_leading_or_trailing_zeros_is_bsr_or_bsf() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("a.c");
    std::fs::write(
        &source,
        "unsigned clz(unsigned a) { return __builtin_clz(a); }\nunsigned ctz(unsigned a) { return __builtin_ctz(a); }\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["--target=x86_32-linux", "-march=i486", "-O2", "-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(text.contains("bsr "), "{text}");
    assert!(text.contains("bsf "), "{text}");
    assert!(!text.contains("1431655765"), "the popcount expansion is back:\n{text}");
}
