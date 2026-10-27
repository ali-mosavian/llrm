//! A value only a select's arm uses is computed in that arm.

mod common;

/// kernels/x_collatz at -Os m32: `x & 1 ? 3 * x + 1 : x >> 1` lowers to a
/// branch over one arm, and the other arm's value (`lea` or `shr`, by the run)
/// stayed ahead of the test (a block on the critical edge costs a jump, so none
/// was made): one more instruction on every trip than the pin's, +8.6% clocks.
#[test]
fn test_the_value_of_a_select_arm_is_not_computed_before_the_test() {
    let source =
        format!("{}/crates/target/llrm-x86-m32/vsgcc/kernels/x_collatz/x_collatz.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-Os", "-m32", "-mabi=sysv", "-march=i486", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    for (at, line) in lines.iter().enumerate() {
        if (line.starts_with("shr ") || line.starts_with("lea "))
            && lines.get(at + 1).is_some_and(|next| next.starts_with("test "))
        {
            panic!("{line} ahead of the test it is the arm of");
        }
    }
}
