//! An x87 constant an inner loop adds is made once ahead of the loop.

mod common;

/// `x_ld_abs` at -O2: the outer loop's four trips unroll, and each copy of
/// `x[i] - 16 - r` made `-1.0` (`fld1; fchs`) and added it on every inner trip,
/// 13,320 clocks against gcc's 13,113. The pin leaves it there because the
/// `fptosi` after the loops writes the control word, which `fld1` reads in its
/// model; the constant does not depend on the rounding control.
const SOURCE: &str = "
extern long double x[64];
long f(int n)
{
    long double t = 0;
    for (int r = 0; r < 4; r++)
        for (int i = 0; i < n; i++) {
            long double v = x[i] - 16 - r;
            t += v < 0 ? -v : v;
        }
    return (long)(t * 16);
}
";

/// Whether line `at` of the listing is inside a loop: a jump after it to a
/// label before it.
fn in_a_loop(
    lines: &[&str],
    at: usize,
) -> bool {
    lines[at + 1..].iter().any(|jump| {
        let mut words = jump.split_whitespace();
        words.next().is_some_and(|op| op.starts_with('j'))
            && words.next().is_some_and(|label| lines[..at].iter().any(|line| line.trim() == format!("{label}:")))
    })
}

#[test]
fn test_a_constant_the_inner_loop_adds_is_not_made_on_every_trip() {
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
    let lines: Vec<&str> = text.lines().collect();
    let made: Vec<usize> = (0..lines.len()).filter(|&at| lines[at].trim() == "fld1").collect();
    assert!(!made.is_empty(), "premise: the unrolled copies make 1.0: {text}");
    assert!(made.iter().all(|&at| !in_a_loop(&lines, at)), "`fld1` is in a loop: {text}");
}
