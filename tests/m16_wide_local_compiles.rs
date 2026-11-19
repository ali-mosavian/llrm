//! A local above the stack alignment compiles at 16 bits.

mod common;

/// `f` was refused with "an over-aligned local in 16-bit code is not
/// realigned" once the over-aligned threshold followed the target's stack
/// alignment (bench/nbody -O2 at m16): real mode cannot realign, and the pin
/// clamps such a local's alignment instead of refusing it.
const SOURCE: &str = "extern void g(double *);\nvoid f(void) { double d[4]; d[0] = 1.0; g(d); }\n";

#[test]
fn test_an_eight_aligned_local_compiles_at_m16() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-march=i486", "-O2", "--isel=dag", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
