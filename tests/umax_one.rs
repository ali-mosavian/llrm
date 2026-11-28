//! `umax(x, 1)` is `cmp x, 1; adc x, 0`, not a compare against 2 and a `cmov`.

mod common;

/// The soft-float exponents (`x ? x : 1`) came out as `cmp e, 2; mov r, 1;
/// cmovae r, e`: 3 instructions and a register where the pin has `mov; cmp;
/// adc`; f64 mul at m64 was 19 bytes over the pin.
#[test]
fn a_maximum_with_one_is_a_carry_add() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), "unsigned f(unsigned x) { return x > 1 ? x : 1; }\n").unwrap();
    for target in ["--target=x86_64-linux", "--target=x86_32-linux"] {
        let output = common::cc::llrm_c()
            .current_dir(directory)
            .args([target, "-O2", "-S", "-o", "a.s", "a.c"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
        assert!(text.contains("adc ") && !text.contains("cmov"), "{target}: {text}");
    }
}
