//! A register no one writes is not saved.

mod common;

fn listing(source: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O1", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// A call of a function that returns a double was taken to destroy EDX, the
/// high half of an 8-byte result, though the double is in st(0): recfloat's
/// `decay` pushed and popped EDX around a body that never wrote it.
#[test]
fn a_call_that_returns_a_double_does_not_disturb_edx() {
    let text = listing(
        "static double decay(double x, double acc, int n) { if (n == 0) return acc; return decay(x * 0.5 + 1.0, acc + x, n - 1); }\n\
         double g(int n) { return decay(100.0, 0.0, n); }\n",
    );
    assert!(!text.contains("push edx") && !text.contains("pop edx"), "{text}");
}
