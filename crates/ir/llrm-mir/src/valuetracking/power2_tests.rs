//! `p2` and `p2z` against LLVM's answers on `tests/vt/nonzero/pow2.ll`.

use super::nonzero_tests::check;

#[test]
#[ignore = "needs computeKnownBits (vt/known)"]
fn powers_of_two_are_as_llvm_says() {
    check(include_str!("../../tests/vt/nonzero/pow2.ll"), include_str!("../../tests/vt/nonzero/pow2.expected"));
}
