//! copy_runs -O2 (1411 B / 206971 T against 1317 B / 203982 T at -O1):
//! `sum(48)` was inlined at two sites, +94 B. Its bound is known, but a trip's
//! body (32 units) times 48 is over the full-unroll threshold, so the copy
//! folds nothing and is as big as the callee: the known-bound exemption holds
//! only where the unrolled body fits.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

#[test]
fn a_known_bound_whose_unrolled_body_is_over_the_threshold_keeps_the_call() {
    let ir = include_str!("data/copy_runs_o2.ll");
    let asm = text(ir, "main", 0, &Request::new(Level::O2)).expect("compiles");
    assert_eq!(asm.matches("call _sum").count(), 2, "sum was inlined into main:\n{asm}");
}
