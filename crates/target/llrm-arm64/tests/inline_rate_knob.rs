//! `LLRM_INL_RATE` (scan knob, w15/rate-scan only) sets the inline rate in
//! milliclocks per byte; unset or unparsable leaves the fitted rate. One test:
//! the environment is process-wide.

use llrm_arm64::tti::AArch64;
use llrm_mir::tti::Tti;

#[test]
fn the_scan_knob_sets_the_inline_rate_and_unset_leaves_the_fitted_one() {
    let rate = || AArch64::of_cpu("generic").unwrap().speed_inline_milliclocks_per_byte();
    // SAFETY: the only test in this binary, so no other thread reads the environment.
    unsafe { std::env::remove_var("LLRM_INL_RATE") };
    assert_eq!(rate(), 62_500, "unset: the fitted 62.5 clocks a byte");
    unsafe { std::env::set_var("LLRM_INL_RATE", "300") };
    assert_eq!(rate(), 300, "set: the knob's value");
    unsafe { std::env::set_var("LLRM_INL_RATE", "fast") };
    assert_eq!(rate(), 62_500, "unparsable: the fitted rate");
    unsafe { std::env::remove_var("LLRM_INL_RATE") };
}
