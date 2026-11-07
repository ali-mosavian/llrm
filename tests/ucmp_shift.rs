//! An unsigned compare with a power of two that is no 32-bit immediate is a
//! shift.

mod common;

/// `x < 2^41` on x86-64 came out as `movabs; cmp; sbb`, not the pin's `shr 41`.
#[test]
fn an_unsigned_compare_with_a_wide_power_of_two_is_a_shift() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), "int f(unsigned long x) { return x < (1UL << 41) ? 1 : 2; }\n").unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["--target=x86_64-linux", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
    assert!(text.contains("shr") && !text.contains("movabs"), "{text}");
}
