//! `x < 0 ? -x : x` into a conversion is `fabs`.

mod common;

/// x_ldcmp -O2 at m32: `(long)fabs_(v[i])` compared with zero, branched and
/// negated on every trip (`fcom`, `fnstsw`, `sahf`, `jbe`, `fchs`): 257,305
/// clocks against gcc's 204,830. `fptosi` reads neither the sign of a zero nor
/// of a NaN, so the pin makes the select `fabs`.
const SOURCE: &str = "
long f(long double x)
{
    return (long)(x < 0 ? -x : x);
}
";

#[test]
fn test_a_sign_test_that_negates_into_a_conversion_is_an_fabs() {
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
    assert!(text.contains("fistp"), "premise: the conversion is in the listing: {text}");
    assert!(text.contains("fabs"), "no fabs: {text}");
    assert!(!text.contains("fnstsw"), "the sign test is still a compare: {text}");
}
