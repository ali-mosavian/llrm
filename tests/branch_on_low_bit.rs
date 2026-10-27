//! `br (trunc x to i1)` is `test x, 1` (X86TargetLowering::LowerBRCOND, TEST8ri
//! X86InstrArithmetic.td#L1330).

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

/// x_collatz's odd/even test was `mov edx, ecx; and dl, 1; jne` at -m32: the
/// copy and the `and` for what the pin makes `test cl, 1; jne` (the dword
/// source was left out of the first translation).
#[test]
fn a_dword_odd_test_branches_on_a_test_of_its_low_bit() {
    let text = listing(
        "extern void odd(int);\nextern void even(int);\nvoid f(int n) { if (n & 1) odd(n); else even(n); }\n",
        &["-m32", "-mabi=sysv", "-O2", "-march=i486"],
    );
    assert!(text.contains("test ") && !text.contains("and dl, 1") && !text.contains("and cl, 1"), "{text}");
}
