//! The copies that run most often are joined first.

mod common;

use std::process::Command;

const KERNEL: &str = "
static int a[512];
long f(int key)
{
    int lo = 0, hi = 511, mid;
    while (lo <= hi) {
        mid = (lo + hi) >> 1;
        if (a[mid] < key) lo = mid + 1;
        else if (a[mid] > key) hi = mid - 1;
        else return mid;
    }
    return -1;
}
";

/// The copy of `lo` into the join of `lo = mid + 1` and `lo = lo` was joined
/// with `mid` (the first copy in block order, an arm that runs half the trips)
/// and not with `lo` (the copy at the join, which runs every trip): the other
/// arm copied `lo` in and out of `mid`'s register and jumped to the join, a
/// `mov`, `mov` and `jmp` a trip in x_binsearch at -O1.
#[test]
fn test_the_join_of_a_loop_variable_takes_its_register_not_the_registers_of_one_arm() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["-m32", "-mabi=sysv", "-O1", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let jumps = listing.lines().map(str::trim).filter(|line| line.starts_with("jmp ")).count();
    assert!(jumps <= 1, "{jumps} unconditional jumps: the arm that updates hi jumps to a join that copies: {listing}");
}
