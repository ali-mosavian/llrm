//! Nine live doubles spill no more than the pin's allocation does.

mod common;

const KERNEL: &str = "
double f9(const double *a)
{
    double x0 = a[0], x1 = a[1], x2 = a[2], x3 = a[3], x4 = a[4];
    double x5 = a[5], x6 = a[6], x7 = a[7], x8 = a[8];
    return x0 * x1 + x1 * x2 + x2 * x3 + x3 * x4 + x4 * x5 + x5 * x6 + x6 * x7 + x7 * x8 + x8 * x0
         + x0 * x4 + x1 * x5 + x2 * x6 + x3 * x7 + x4 * x8;
}
";

/// The spill of each piece of a split x87 value stored the piece into a home
/// of its own: ten stores to the frame where LLVM's greedy allocation and
/// spill hoisting store seven (`InlineSpiller::hoistSpillInsideBB`,
/// `eliminateRedundantSpills`). Staged: 7 is the pin's number with the machine
/// scheduler off (5 with it); main's floatassign stores 3 (X87-GAP-4), the
/// bound this test moves to when that row lands.
#[test]
fn test_nine_live_doubles_spill_no_more_than_the_pin_through_the_frame() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output =
        common::cc::llrm_cc().current_dir(directory).args(["-m32", "-O2", "-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let stores = listing
        .lines()
        .map(str::trim)
        .filter(|line| (line.starts_with("fst ") || line.starts_with("fstp ")) && line.contains("[esp"))
        .count();
    assert!(stores <= 7, "{stores} stores to the frame: {listing}");
}
