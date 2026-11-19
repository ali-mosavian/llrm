//! A loop that calls an extended-precision intrinsic is sized, so it unrolls.

mod common;

/// `llvm.fmuladd.f80` had no price: the loop's size was not computed and
/// neither it nor the loop around it was unrolled. x_ld_to_int ran a `fiadd`
/// of its outer counter (19 clocks) in every inner trip: 24,269 clocks against
/// 20,730 for gcc.
const SOURCE: &str = "long double x[64];\n\
    long f(int n)\n\
    {\n\
    \x20   long t = 0;\n\
    \x20   for (int r = 0; r < 4; r++)\n\
    \x20       for (int i = 0; i < n; i++)\n\
    \x20           t += (int)(x[i] * 3.7L - 20 + r);\n\
    \x20   return t;\n\
    }\n";

#[test]
fn test_a_loop_nest_with_a_fused_extended_multiply_is_unrolled_around_its_inner_loop() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args([
            "--target=x86_32-wat",
            "-m32",
            "-mabi=sysv",
            "-mlong-double-80",
            "-march=i486",
            "-O2",
            "-S",
            "-o",
            "a.s",
            "a.c",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(!text.contains("fiadd"), "the outer counter is added to the inner values by memory: {text}");
}
