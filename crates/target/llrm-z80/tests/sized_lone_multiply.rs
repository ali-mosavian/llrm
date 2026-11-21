//! A function tuned for size with one widening multiply was never selected with the routine's call: the
//! product of two sign-extended shorts was four `__mulhi3` calls and their sums. `dot` -Os was 504 B / 8,043,477 T
//! against 341 B / 7,297,501 T at -O1, and over the SDCC bytes bar (479 B). Fixture: the -Os front-end IR of
//! `bench/dot`.

use llrm_z80::compile::{effective, module_priced};
use llrm_z80::driver::{Level, Request};

fn priced(ir: &str, level: Level) -> (u32, u64) {
    let request = effective(ir, &Request::new(level)).expect("chooses");
    module_priced(ir, &request).expect("prices")
}

/// The module of `dot` at -Os with the call is 340 B; with the partial products it is 504 B.
#[test]
fn dot_at_os_calls_the_multiply_routine() {
    let ir = include_str!("data/dot_widening_product_os.ll");
    let os = priced(ir, Level::Os);
    assert!(os.0 <= 400, "-Os {os:?}: the product is partial products again");
}
