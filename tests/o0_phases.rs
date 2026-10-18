//! -O0 improves nothing after selection and allocation, as gcc's and clang's:
//! the backend's optional phases (peephole, coalescing, block placement and
//! copying, the reads folded at their use) are left out, which was 48% of
//! qbq-llrm's d_faces.c compile at -O0 (1031 M instructions against gcc's 256).

use std::process::Command;

fn listing(level: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), "int f(int x) { if (x) return 1; return 2; }\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(["-m32", level, "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// The peephole turned `cmp eax, 0` into `test eax, eax` at every level, -O0
/// included, and was 22% of the compile there.
#[test]
fn o0_runs_no_peephole_and_o1_does() {
    let (none, some) = (listing("-O0"), listing("-O1"));
    // The compare against zero stays a `cmp` (whatever it reads: the branch
    // tests the comparison's own `i1`, so the load is its memory operand).
    let compares_to_zero =
        |text: &str| text.lines().any(|line| line.trim().starts_with("cmp ") && line.trim().ends_with(", 0"));
    assert!(compares_to_zero(&none) && !none.contains("test eax, eax"), "{none}");
    assert!(some.contains("test eax, eax"), "{some}");
}
