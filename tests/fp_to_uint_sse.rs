//! An unsigned conversion as wide as the pointer, from an SSE float, is the
//! pin's `cvttsd2si` pair joined by the overflow sign (`LowerFP_TO_INT`).

mod common;

/// `(unsigned long long)double` was a compare, a branch around a zeroed
/// subtrahend, a `setae` and a shift (double_to_uint64 -O2 m64: 280 insns
/// against the pin's 226).
#[test]
fn test_a_double_to_unsigned_64_has_no_branch_at_m64() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), "unsigned long long f(double d) { return (unsigned long long)d; }\n")
        .unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["--target=x86_64-linux", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(text.contains("sar rax, 63") || text.contains("sar rdx, 63"), "{text}");
    assert!(!text.contains("jae") && !text.contains("setae"), "{text}");
}
