//! A loop of constant trips whose body folds through `llvm.fmuladd` was kept
//! rolled at -Os (the estimator charged the call): x87_arguments grew
//! 37 -> ~258 B of .text at m64.

mod common;

#[test]
fn a_loop_that_folds_through_a_call_is_copied_out_at_os() {
    let scratch = tempfile::tempdir().unwrap();
    let source = "extern void report(long v);\nint main(void)\n{\n    double a = 100.0;\n    int i;\n    for (i = 0; i < 5; ++i) a = a * 0.5 + 1.0;\n    report((long)a);\n    return 0;\n}\n";
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m32", "-Os", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(!listing.contains("fmul") && !listing.contains("fadd"), "the loop is kept: {listing}");
}
