//! A `double` or `float` read into the x87 is the load's own extension
//! (`fld m64`, `fadd m64`), not an SSE load stored and reloaded.

mod common;

/// `a + *p` with `p` a `double *` was `movsd xmm0, [p]; movsd [rsp], xmm0;
/// fadd [rsp]` and a 16-byte frame (dsum -Os m64: 16 insns against the pin's
/// 12).
#[test]
fn test_a_double_added_to_a_long_double_is_read_from_its_cell() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "long double f(long double a, double *p) { return a + *p; }\nlong double g(float *p) { return *p; }\n",
    )
    .unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["--target=x86_64-linux", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(!text.contains("movsd") && !text.contains("movss"), "{text}");
    assert!(text.contains("fadd qword ptr [rdi]") && text.contains("fld dword ptr [rdi]"), "{text}");
    assert!(!text.contains("sub rsp"), "{text}");
}
