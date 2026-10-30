//! A variable i64 shift computes both halves before its branch.

mod common;

/// `a << (b & 63)` at m16 sank `shld` into the false arm (the sink did not see
/// the count's `cl` pin the pin's `$cl = COPY` shows), which made a `jmp` over
/// it: 3 instructions more than the pin's `shld; shl; test; je; mov; xor`.
#[test]
fn test_a_variable_i64_shift_has_no_jump() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), "long long f(long long a, long long b) { return a << (b & 63); }\n")
        .unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["a.c", "-m16", "-march=i386", "-O2", "--isel=dag", "-S", "-o", "-"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = String::from_utf8_lossy(&output.stdout);
    let shld = asm.find("shld").expect("a shld");
    let test = asm.find("test cl").expect("a test of the count");
    assert!(shld < test && !asm.contains("jmp"), "{asm}");
}
