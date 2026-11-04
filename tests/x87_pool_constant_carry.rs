//! A loop-invariant extended constant is loaded once, ahead of the loop.

mod common;

/// `3.7L` was loaded from the pool on every trip (`fld tbyte` 6 clk and a
/// popping `fmulp`); gcc keeps it on the x87 stack. x_ld_to_int -O2 spent 3,533
/// of 18,000 clocks on it.
const SOURCE: &str = "long double f(long double *a, int n) {\n\
    long double s = 0;\n\
    for (int i = 0; i < n; i++) s += a[i] * 3.7L;\n\
    return s;\n\
}\n";

#[test]
fn test_an_invariant_extended_constant_is_loaded_once_before_the_loop() {
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
    let code: Vec<&str> = text.lines().map(str::trim).skip_while(|line| !line.starts_with("_f proc")).collect();
    // The loop: from a label to the jump back to it.
    let (head, tail) = code
        .iter()
        .enumerate()
        .find_map(|(at, line)| {
            let target = line.strip_prefix('j')?.split_whitespace().nth(1)?;
            let head = code[..at].iter().position(|label| label.strip_suffix(':') == Some(target))?;
            Some((head, at))
        })
        .expect(&text);
    assert!(code[head..tail].iter().any(|line| line.starts_with("fmul")), "{text}");
    assert!(
        !code[head..tail].iter().any(|line| line.starts_with("fld tbyte ptr $K")),
        "the constant is loaded on every trip: {text}"
    );
    assert_eq!(code.iter().filter(|line| line.starts_with("fld tbyte ptr $K")).count(), 1, "{text}");
}
