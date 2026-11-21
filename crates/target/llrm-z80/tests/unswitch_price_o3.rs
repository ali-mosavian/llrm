//! DEVIATE UNROLL-SPEED-TRIAL: at -O3 the driver prices a non-trivial unswitch
//! against the module without it. queens -O3 copied the loop of `place` for one
//! invariant branch: 432 B / 4357696 T against -O2's 339 B / 4248951 T, 90 B
//! over SDCC's 342 B.

use llrm_transforms::trial::SpeedUnroll;
use llrm_z80::compile::{effective, module_prices};
use llrm_z80::driver::{Level, Request};

const QUEENS: &str = include_str!("data/queens_o3.ll");

#[test]
fn an_unswitch_that_only_grows_the_module_is_refused() {
    let request = Request::new(Level::O3);
    let chosen = effective(QUEENS, &request).expect("chooses");
    assert_eq!(chosen.unroll_variant, Some(SpeedUnroll::NoUnswitch));
    let (bytes, clocks) = module_prices(QUEENS, &chosen).unwrap();
    let (grown_bytes, grown_clocks) = module_prices(QUEENS, &request).unwrap();
    assert!(bytes + 40 <= grown_bytes && clocks <= grown_clocks, "{grown_bytes} B / {grown_clocks} T kept as {bytes} B / {clocks} T");
}
