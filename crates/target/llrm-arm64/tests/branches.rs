//! How the selector branches.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn asm() -> String {
    driver::assembly(include_str!("data/or_of_compares.ll"), Platform::Darwin, true).expect("the functions lower")
}

/// `n == 0 || m == 0` made a bit of each comparison (subs, cset, subs, cset, orr, tst, b.ne: seven
/// instructions); clang branches on each in turn (tests/llc rectwo: 196 B against 132).
#[test]
fn a_chain_of_comparisons_is_a_chain_of_branches() {
    let asm = asm();
    for set in ["cset", "csinc", "orr", "and\t", "ands"] {
        assert!(!asm.contains(set), "{set} left in\n{asm}");
    }
}

/// An `or` over two comparisons with zero is two `cbz`.
#[test]
fn an_or_of_zero_tests_is_two_compare_branches() {
    let asm = asm();
    let either = asm.split("_both:").next().unwrap();
    assert_eq!(either.matches("cbz").count(), 2, "{either}");
}
