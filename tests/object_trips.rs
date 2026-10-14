//! A loop's trips are bounded by the object its inbounds accesses walk.

use std::process::Command;

/// `dot` walks `A[i][k]` and `B[k][j]` of two 12 x 12 arrays for a run-time
/// `n`; `through` walks pointers of unknown extent.
const KERNEL: &str = "
static double A[12][12], B[12][12];
double dot(int n, int i, int j)
{
    double sum = 0;
    int k;
    for (k = 0; k < n; k++)
        sum += A[i][k] * B[k][j];
    return sum;
}
double through(double *a, double *b, int n)
{
    double sum = 0;
    int k;
    for (k = 0; k < n; k++)
        sum += a[k] * b[k];
    return sum;
}
";

fn multiplies(level: &str) -> (usize, usize) {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(["-m32", level, "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let split = listing.find("through_ proc").expect("through is in the listing");
    let (dot, through) = listing.split_at(split);
    (dot.matches("fmul").count(), through.matches("fmul").count())
}

/// x_matmul_f's `k` loop stayed rolled at -O3 with 78,749 clocks where gcc's
/// copies of it, 12 of them (`infer_loop_bounds_from_array`), took 67,907:
/// the trips were unknown to `peel`, though `B[k][j]` cannot go past row 11.
/// Pointers of unknown extent promise nothing, and -O2 copies nothing that
/// grows the code.
#[test]
fn test_o3_copies_out_a_loop_over_arrays_by_their_extent_and_only_that() {
    assert_eq!(multiplies("-O3"), (12, 1));
    assert_eq!(multiplies("-O2"), (1, 1));
}
