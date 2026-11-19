//! x87 arithmetic does not touch EFLAGS: a loop counter's `dec` and the `jne`
//! that reads it can have `fmul` and `fadd` between them.

mod common;

/// Horner's rule over a descending index. `dec` of the counter was made a
/// `lea` (its flags taken for dead past the `fmul`), so the loop's `jne` read
/// the flags of an earlier `test`: the loop ran past the array (a
/// long double kernel at m32 -O2 answered 328 for 7059).
const SOURCE: &str = "double horner(const double *p, int n, double x) {\n\
    double r = 0;\n    for (int i = n - 1; i >= 0; i--) r = r * x + p[i];\n    return r;\n}\n";

#[test]
fn test_a_loop_branch_reads_flags_an_instruction_before_it_set() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["--target=x86_32-wat", "-m32", "-mabi=sysv", "-march=i486", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let branch = lines.iter().rposition(|line| line.starts_with("jne ")).expect("the loop branch");
    let head = lines[..branch].iter().rposition(|line| line.ends_with(':')).unwrap();
    let sets_flags = lines[head..branch]
        .iter()
        .any(|line| ["dec ", "inc ", "sub ", "add ", "cmp ", "test "].iter().any(|op| line.starts_with(op)));
    assert!(sets_flags, "nothing in the loop sets the flags its `jne` reads: {text}");
}
