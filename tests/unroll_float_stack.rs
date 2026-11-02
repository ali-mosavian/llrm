//! A full unroll on a float stack is the pin's: no cap by the stack depth.

mod common;

fn adds(source: &str) -> usize {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m32", "-O3", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap().matches("fadd").count()
}

/// Five sums live at once in an 8-trip loop. A decline by float-stack depth
/// left fpbench and nbody_single 18% more instructions than the pin's unroll
/// (bq32 -O2, vs main).
#[test]
fn five_floats_live_in_a_body_are_copied_out_as_the_pin() {
    let source = "float f(const float *a, const float *b)\n{\n    float s0 = 0, s1 = 0, s2 = 0, s3 = 0, s4 = 0;\n    int i;\n    for (i = 0; i < 8; i++) {\n        s0 += a[i] * b[i]; s1 += a[i] * s0; s2 += b[i] * s1; s3 += a[i] * s2; s4 += b[i] * s3;\n    }\n    return s0 + s1 + s2 + s3 + s4;\n}\n";
    let count = adds(source);
    assert!(count >= 40, "{count} fadd: the 8 trips stayed a loop");
}

/// A Horner chain keeps two live: the unroll is the 15% x_horner won.
#[test]
fn a_chain_of_few_floats_is_still_copied_out() {
    let source = "double c[8];\ndouble f(double x)\n{\n    double p = 0;\n    int k;\n    for (k = 0; k < 8; k++) p = p * x + c[k];\n    return p;\n}\n";
    let count = adds(source);
    assert!(count >= 8, "{count} fadd: the chain stayed a loop");
}
