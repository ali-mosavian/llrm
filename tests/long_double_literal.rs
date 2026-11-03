//! A `long double` literal keeps its 64 significand bits.

mod common;

/// `return 0.1L` loaded the double 0.1 (`fld qword`, bytes 9a 99 99 99 99 99 b9
/// 3f) on every x87 target: the pin's clang writes the constant as the decimal
/// `1.000000e-01` in the IR, and the reader rounded it to a double.
const SOURCE: &str = "long double f(void) { return 0.1L; }\n";

fn assembled(target: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(target)
        .args(["-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

#[test]
fn test_a_decimal_long_double_literal_is_a_ten_byte_constant() {
    for target in
        [&["--target=x86_32-linux"][..], &["--target=x86_64-linux"], &["--target=x86_32-wat", "-mlong-double-80"]]
    {
        let text = assembled(target);
        // 0.1L: significand CCCCCCCCCCCCCCCD, exponent 3FFB.
        assert!(text.contains("0cdh,0cch,0cch,0cch,0cch,0cch,0cch,0cch,0fbh,03fh"), "{target:?}: {text}");
        assert!(text.contains("fld tbyte ptr"), "{target:?}: {text}");
    }
}
