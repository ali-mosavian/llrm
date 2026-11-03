//! A full unroll on a float stack: copies of a body whose floats do not fit
//! the stack twice are interleaved past it and spill.

mod common;

fn adds(
    source: &str,
    level: &str,
) -> usize {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m32", level, "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap().matches("fadd").count()
}

/// Five sums live at once in a 16-trip loop: fpbench, nbody at -O3 were 2.4%
/// slower than their loop, 53 spills for 6, once unrolled past the default
/// bar (a body that fits the default bar is copied out: the next test).
#[test]
fn five_floats_live_in_a_body_keep_the_loop() {
    let source = "float f(const float *a, const float *b)\n{\n    float s0 = 0, s1 = 0, s2 = 0, s3 = 0, s4 = 0;\n    int i;\n    for (i = 0; i < 16; i++) {\n        s0 += a[i] * b[i]; s1 += a[i] * s0; s2 += b[i] * s1; s3 += a[i] * s2; s4 += b[i] * s3;\n    }\n    return s0 + s1 + s2 + s3 + s4;\n}\n";
    let count = adds(source, "-O3");
    assert!(count < 40, "{count} fadd: the 16 trips were copied out");
}

/// A Horner chain keeps two live: the unroll is the 15% x_horner won.
#[test]
fn a_chain_of_few_floats_is_still_copied_out() {
    let source = "double c[8];\ndouble f(double x)\n{\n    double p = 0;\n    int k;\n    for (k = 0; k < 8; k++) p = p * x + c[k];\n    return p;\n}\n";
    let count = adds(source, "-O3");
    assert!(count >= 8, "{count} fadd: the chain stayed a loop");
}

/// The same five sums over 4 trips at -O2: inside the default unroll bar the
/// unrolled loop won (nbody_single -2.5% and fpbench -4.0% at -O2 bq32); the
/// gate had refused it at every level.
#[test]
fn five_floats_live_in_a_small_body_are_copied_out_at_o2() {
    let source = "float f(const float *a, const float *b)\n{\n    float s0 = 0, s1 = 0, s2 = 0, s3 = 0, s4 = 0;\n    int i;\n    for (i = 0; i < 4; i++) {\n        s0 += a[i] * b[i]; s1 += a[i] * s0; s2 += b[i] * s1; s3 += a[i] * s2; s4 += b[i] * s3;\n    }\n    return s0 + s1 + s2 + s3 + s4;\n}\n";
    let count = adds(source, "-O2");
    assert!(count >= 20, "{count} fadd: the 4 trips stayed a loop");
}
