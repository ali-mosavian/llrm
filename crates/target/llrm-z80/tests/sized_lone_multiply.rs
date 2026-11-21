//! A function tuned for size with one widening multiply was never selected with
//! the routine's call: the product of two sign-extended shorts was four
//! `__mulhi3` calls and their sums. `dot` -Os was 504 B / 8,043,477 T
//! against 341 B / 7,297,501 T at -O1, and over the SDCC bytes bar (479 B).
//! Fixture: the -Os front-end IR of `bench/dot`.

use llrm_z80::compile::{effective, module_priced};
use llrm_z80::driver::{Level, Request};

fn priced(
    ir: &str,
    level: Level,
) -> (u32, u64) {
    let request = effective(ir, &Request::new(level)).expect("chooses");
    module_priced(ir, &request).expect("prices")
}

/// The module of `dot` at -Os with the call is 340 B; with the partial products
/// it is 504 B.
#[test]
fn dot_at_os_calls_the_multiply_routine() {
    let ir = include_str!("data/dot_widening_product_os.ll");
    let os = priced(ir, Level::Os);
    assert!(os.0 <= 400, "-Os {os:?}: the product is partial products again");
}

/// A function with a long divide has the speed routines as its one alternative.
/// Giving it the lone multiply's call instead took them away: copy_runs -Os
/// went from 953 B / 204,454 T to 767 B / 729,429 T.
#[test]
fn copy_runs_at_os_keeps_the_speed_routine_of_its_divide() {
    let ir = include_str!("data/copy_runs_divide_os.ll");
    let os = priced(ir, Level::Os);
    // the speed routine is the larger one: 953 B against the size routine's 767
    // B (the model reads the size routine as the faster, 250,034 clocks
    // against 328,067; ucsim says 3.6x slower, see
    // z80-module-clock-estimate.md) 24 B of the 953 are LSR's (the
    // alternate order, LSR-KEY-TRIAL: 929 B, the same `__modsi3` call); the
    // size routine's module is 767 B
    assert!(os.0 >= 900, "-Os {os:?}: the divide lost its speed routine");
}
