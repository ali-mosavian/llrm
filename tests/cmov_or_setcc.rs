//! A select on `a || b` of two compares is two `cmov`s off the same flags.

mod common;

/// The soft-float NaN result (`x == y || z == w ? NaN : r`) came out as `sete
/// al; sete cl; or cl, al; cmovne`; the pin has two `cmove`s, no `or`.
#[test]
fn a_select_on_two_compares_ored_is_two_conditional_moves() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(
        directory.join("a.c"),
        "long f(long a, long b, long c, long d, long v) { return (a == b || c == d) ? 99999999999L : v; }\n",
    )
    .unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["--target=x86_64-linux", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
    assert_eq!(text.matches("cmov").count(), 2, "{text}");
    assert!(!text.contains("sete") && !text.contains("or c"), "{text}");
}
