//! The loop-idiom pass is named `loopidioms` in the `LLRM_DEBUG=runs` trace,
//! not `fill`: a zero-fill loop at -O2 ran steps `r01-fill`, `r02-fill`.

mod common;

#[test]
fn test_the_trace_names_the_idiom_pass_loopidioms_not_fill() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(
        directory.join("a.c"),
        "unsigned char buf[100];\nvoid f(void) { int i; for (i = 0; i < 100; ++i) buf[i] = 0; }\n",
    )
    .unwrap();
    let output = common::cc::llrm_cc()
        .current_dir(directory)
        .env("LLRM_DEBUG", "runs")
        .args(["-m32", "-march=i486", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let trace = String::from_utf8_lossy(&output.stderr);
    let steps: Vec<&str> = trace.lines().filter_map(|line| line.strip_prefix("[runs] step ")).collect();
    let named = |name: &str| {
        steps.iter().any(|step| {
            step.split(' ').next().is_some_and(|one| one.split('@').next().is_some_and(|pass| pass.ends_with(name)))
        })
    };
    assert!(named("-loopidioms"), "no loopidioms step in:\n{trace}");
    assert!(!named("-fill"), "a step still named fill in:\n{trace}");
}
