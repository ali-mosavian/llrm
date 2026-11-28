//! DEVIATE UNROLL-SPEED-TRIAL: at -O3 the driver prices a non-trivial unswitch
//! against the module without it. queens -O3 copied the loop of `place` for one
//! invariant branch: 432 B / 4357696 T against -O2's 339 B / 4248951 T, 90 B
//! over SDCC's 342 B. The variant is kept only where the module is smaller and
//! no slower: loop_versioning -O3 is 308 B smaller without its unswitch and 69%
//! slower, and keeps it.

use llrm_transforms::trial::SpeedUnroll;
use llrm_z80::compile::{effective, module_priced};
use llrm_z80::driver::{Level, Request};

const QUEENS: &str = include_str!("data/queens_o3.ll");
const BUYS_CLOCKS: &str = include_str!("data/unswitch_buys_clocks_o3.ll");

#[test]
fn an_unswitch_that_only_grows_the_module_is_refused() {
    let request = Request::new(Level::O3);
    let chosen = effective(QUEENS, &request).expect("chooses");
    assert_eq!(chosen.unroll_variant, Some(SpeedUnroll::NoUnswitch));
    let (bytes, clocks) = module_priced(QUEENS, &chosen).unwrap();
    let (grown_bytes, grown_clocks) = module_priced(QUEENS, &request).unwrap();
    assert!(
        bytes + 40 <= grown_bytes && clocks <= grown_clocks,
        "{grown_bytes} B / {grown_clocks} T kept as {bytes} B / {clocks} T"
    );
}

#[test]
fn an_unswitch_that_buys_clocks_is_kept_where_the_module_is_smaller_without_it() {
    let request = Request::new(Level::O3);
    let chosen = effective(BUYS_CLOCKS, &request).expect("chooses");
    assert_ne!(chosen.unroll_variant, Some(SpeedUnroll::NoUnswitch));
    let (bytes, clocks) = module_priced(BUYS_CLOCKS, &chosen).unwrap();
    let without = Request { unroll_variant: Some(SpeedUnroll::NoUnswitch), ..request };
    let (other_bytes, other_clocks) = module_priced(BUYS_CLOCKS, &without).unwrap();
    assert!(
        other_bytes < bytes && other_clocks > clocks,
        "premise: {bytes} B / {clocks} T against {other_bytes} B / {other_clocks} T"
    );
}
