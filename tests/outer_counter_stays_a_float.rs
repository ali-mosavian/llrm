//! An outer loop's counter that an inner loop converts to float is a float.

mod common;

/// `x_ld_div` at -Os: the outer counter `r` was converted by `fiadd [r]` in
/// every inner trip (19+ clocks on a 486) because LSR skipped outer loops
/// before it looked for shadow variables; the pin looks first and keeps `r`
/// counting in a register beside the integer. 29,293 clocks against clang's
/// 26,528.
const SOURCE: &str = "
static long double x[64], y[64];
long f(int n)
{
    long double t = 0;
    for (int r = 0; r < 4; r++)
        for (int i = 0; i < n; i++) t += x[i] / (y[i] + r);
    return (long)(t * 16);
}
";

#[test]
fn test_the_outer_counter_is_not_converted_in_every_inner_trip() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["--target=x86_32-wat", "-m32", "-mabi=sysv", "-mlong-double-80", "-march=i486", "-Os", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(text.contains("fdiv"), "premise: the division is in the listing: {text}");
    assert!(!text.contains("fiadd"), "the inner loop converts the integer counter: {text}");
}
