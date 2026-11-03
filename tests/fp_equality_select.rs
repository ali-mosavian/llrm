//! `a == b ? x : y` on floats is `cmovne` then `cmovp` on the one compare
//! (`combineCMov`), not two `setcc`s, an `and` and a test.

mod common;

/// The select was `setnp; sete; and; cmovne` (4 insns for the condition
/// against the pin's 2 cmovs); the `!=` form likewise with `or`.
#[test]
fn test_a_float_equality_select_is_two_cmovs() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "int eq(double a, double b, int x, int y) { return a == b ? x : y; }\nint ne(double a, double b, int x, int y) { return a != b ? x : y; }\n",
    )
    .unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["--target=x86_64-linux", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert_eq!(text.matches("cmovp").count(), 2, "{text}");
    assert!(!text.contains("setnp") && !text.contains("setp"), "{text}");
}
