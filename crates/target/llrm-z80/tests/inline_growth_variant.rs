//! The variant that takes no inline growth refuses the SZ2 trial's too
//! (`NoInlineGrowthAll`), and the one that keeps the trial's (`NoInlineGrowth`)
//! stays: the module price picks between them.
//!
//! shift_int64_masked_amount -O2 inlined `mix` at three hot sites, its long
//! multiplies became `__mulsi3` calls: 893 B / 4276255 T against 812 B /
//! 3559721 T at -O1. shrink_wrapped_frame -O3 is 374 B / 6014 T only through
//! the SZ2 inlines the policy refused (456 B / 49225 T without them).

use llrm_z80::compile::{effective, module_priced};
use llrm_z80::driver::{Level, Request};

fn priced(
    ir: &str,
    level: Level,
) -> (u32, u64) {
    let request = effective(ir, &Request::new(level)).expect("chooses");
    module_priced(ir, &request).expect("prices")
}

#[test]
fn shift_masked_amount_o2_keeps_the_call_of_mix() {
    let ir = include_str!("data/shift_masked_amount_o2.ll");
    let (o1, o2) = (priced(ir, Level::O1), priced(ir, Level::O2));
    assert!(o2.0 <= o1.0 && o2.1 <= o1.1, "-O2 {o2:?} against -O1 {o1:?}");
}

#[test]
fn shrink_wrapped_frame_o3_keeps_the_inlines_the_trial_takes() {
    let (bytes, clocks) = priced(include_str!("data/shrink_wrapped_frame_o3.ll"), Level::O3);
    assert!(bytes <= 380 && clocks <= 10_000, "-O3 {bytes} B / {clocks} T");
}
