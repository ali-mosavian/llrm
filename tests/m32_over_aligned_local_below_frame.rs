//! A 16-aligned local in a 32-bit frame register frame sits on a 16 boundary.

mod common;

/// At -O0 `f` put `m` at `[ebp-36]` with the stack only 4-aligned: the 16
/// boundary was never kept (the over-aligned test was a constant 16, above what
/// the frame keeps), so `m & 15` was 4 and a `movaps` would fault.
const SOURCE: &str = "extern void g(int *);\n\
    long f(void) { int m[8] __attribute__((aligned(16))); for (int j = 0; j < 8; ++j) m[j] = 0; g(m); return (long)m & 15; }\n";

#[test]
fn test_a_16_aligned_local_at_m32_realigns_the_stack() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .args(["-O0", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    // The stack is realigned, and the array sits a multiple of 16 above the
    // aligned stack pointer.
    assert!(text.contains("and esp, -16"), "the stack is not realigned: {text}");
    let offset: i64 = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("lea ")?.split("[esp+").nth(1)?.trim_end_matches(']').parse().ok())
        .unwrap_or_else(|| panic!("no lea of the array off esp: {text}"));
    assert_eq!(offset % 16, 0, "the array lies at esp+{offset}: {text}");
}
