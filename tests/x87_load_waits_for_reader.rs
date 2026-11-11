//! A float load waits for its reader where that saves an exchange.

mod common;

/// `fld x; fld y; fxch; fmul K; faddp`: the load of `y` sat below the multiply
/// that wants `x` on top. 512 exchanges (2,048 clocks) in x_ld_axpy -O3.
const SOURCE: &str = "long double x[64], y[64];\n\
    void f(int n) { for (int i = 0; i < n; i++) y[i] += 0.25L * x[i]; }\n";

fn listing(march: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["--target=x86_32-wat", "-m32", "-mabi=sysv", "-mlong-double-80", march, "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

#[test]
fn test_the_load_of_the_added_value_waits_for_the_add_on_the_486() {
    let text = listing("-march=i486");
    assert!(text.contains("fmul dword ptr"), "{text}");
    assert!(!text.contains("fxch"), "{text}");
}

/// The P5 pairs `fxch`: it prices 1 clock there and the code keeps its order.
#[test]
fn test_the_pentium_keeps_the_order() {
    assert!(listing("-march=pentium").contains("fxch"));
}
