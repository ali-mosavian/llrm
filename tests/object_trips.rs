//! A loop over arrays: LLVM bounds no trips by the object its accesses walk.

mod common;

/// `dot` walks `A[i][k]` and `B[k][j]` of two 12 x 12 arrays for a run-time
/// `n`; `through` walks pointers of unknown extent.
const KERNEL: &str = "
double A[12][12], B[12][12];
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
    let output = common::cc::llrm_c()
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
/// copies of it, 12 of them (`infer_loop_bounds_from_array`), took 67,907, and
/// the pass copied it out by the arrays' extent. LLVM's LoopUnroll takes no
/// trip bound from an inbounds access to an array, so the translated pass
/// leaves `dot` rolled at both levels as the pin does (clang -m32 -O3
/// -march=i486 on this kernel with the arrays external: one `fmul` in `dot` and
/// one in `through`).
#[test]
fn test_o3_leaves_a_loop_over_arrays_rolled_as_the_pin_does() {
    assert_eq!(multiplies("-O3"), (1, 1));
    assert_eq!(multiplies("-O2"), (1, 1));
}
