//! The inliner prices growth in cost units; arm64's unit is an instruction.

use llrm_arm64::tti::AArch64;
use llrm_mir::tti::Tti;

/// The inline byte rate was applied to cost units as if each were a byte (the
/// default unit), though an arm64 instruction is 4 B: jpeg_fixed -O3 grew
/// ~90 B a site against the 18 the comparison saw. The unit is stated; the
/// speed rate is off (the pin's threshold alone at -O2/-O3), so no price is
/// charged per unit.
#[test]
fn a_unit_of_inline_growth_is_four_bytes_and_no_speed_rate_is_charged() {
    let arm = AArch64::of_cpu("generic").unwrap();
    assert_eq!(arm.inline_o1_unit_bytes_milli(), 4000);
    assert_eq!(arm.speed_inline_milliclocks_per_byte(), 0);
}
