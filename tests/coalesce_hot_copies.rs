//! The copies that run most often are joined first.

mod common;

use std::path::Path;

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

/// Register-to-register `mov`s of `text`.
fn register_copies(text: &str) -> usize {
    let register = |name: &str| matches!(name, "eax" | "ebx" | "ecx" | "edx" | "esi" | "edi" | "ebp");
    common::pin::instructions(text, Some("f"))
        .iter()
        .filter(|line| {
            line.strip_prefix("mov ")
                .and_then(|rest| rest.split_once(", "))
                .is_some_and(|(to, from)| register(to) && register(from))
        })
        .count()
}

/// The copy of `lo` into the join of `lo = mid + 1` and `lo = lo` was joined
/// with `mid` (the first copy in block order, an arm that runs half the trips)
/// and not with `lo` (the copy at the join, which runs every trip): the other
/// arm copied `lo` in and out of `mid`'s register and jumped to the join, a
/// `mov`, `mov` and `jmp` a trip in x_binsearch at -O1.
///
/// llrm-cc: the jumps were llrm-c's layout; the pin's `llc` on the same IR
/// also duplicates the loop test into the arm, which this allocator does not
/// (reported, not asserted). The copies are the observable: the function holds
/// no more register-to-register `mov`s than the pin's (rewritten on observable
/// output).
#[test]
fn test_the_join_of_a_loop_variable_takes_its_register_not_the_registers_of_one_arm() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("a.c");
    std::fs::write(&source, KERNEL).unwrap();
    let flags = ["-m32", "-mabi=sysv", "-O1", "-march=i486"];
    let output = common::cc::llrm_cc()
        .current_dir(scratch.path())
        .args(flags)
        .args(["-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let pin = common::pin::listing(Path::new(&source), &flags);
    let (ours, pins) = (register_copies(&listing), register_copies(&pin));
    assert!(pins > 0, "premise: the pin's loop copies a register: {pin}");
    assert!(ours <= pins, "{ours} register copies, the pin's llc has {pins}: {listing}");
}
