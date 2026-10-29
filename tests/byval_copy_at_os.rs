//! A by-value aggregate at m16 -Os is pushed, not copied by string moves.

mod common;

/// `getMemcpy` with AlwaysInline (a byval argument) uses the pin's
/// MaxStoresPerMemcpyOptSize, 4, not m16's 2 for a plain `memcpy` (SD225: a
/// dword load and store pair against `rep movs`). Putting this copy on m16's
/// table made three dwords a 12-byte frame, a `rep movsw` and its setup: 28
/// bytes against the 24 of three `push dword ptr`s.
#[test]
fn at_os_a_three_dword_byval_argument_is_three_pushes() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    let source = "struct S { long a, b, c; } __attribute__((aligned(4)));\nextern struct S g;\nextern void use(struct S s);\nvoid f(void) { use(g); }\n";
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = common::cc::llrm_cc()
        .current_dir(directory)
        .args(["-Os", "-m16", "-march=i386", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
    assert!(!text.contains("rep movs"), "{text}");
    assert_eq!(text.matches("push dword ptr").count(), 3, "{text}");
}
