//! A loop of a known number of trips that may also leave early.

mod common;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output =
        common::cc::llrm_c().current_dir(directory).args(flags).args(["-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

const SEARCH: &str = "int f(const int *q, int col)\n{\n    int r;\n    for (r = 0; r < 3; ++r)\n        if (q[r] == col) return 0;\n    return 1;\n}\n";

/// `for (r = 0; r < 3; ++r) if (q[r] == col) return 0;` stayed a loop at -O3:
/// the counted exit gave no count for a loop with another exit. gcc's -O3
/// copies it by the bound of its trips (`loop_max_iterations`), and queens'
/// `safe` with `row` known is such a loop: gcc's eight clones of `place` were
/// unrolled, ours were not (141608 instructions against gcc's 77864).
#[test]
fn a_loop_of_three_trips_that_may_leave_early_is_copied_out_at_o3() {
    let text = listing(SEARCH, &["-m32", "-O3", "-march=i486"]);
    // One compare for each trip; the first reads q[0] into a register
    // first (`mov ebx, [eax]; cmp ebx, edx`) where the pin compares memory.
    let compares = text.lines().filter(|line| line.trim().starts_with("cmp ")).count();
    assert_eq!(compares, 3, "one compare of q[r] for each trip: {text}");
}
