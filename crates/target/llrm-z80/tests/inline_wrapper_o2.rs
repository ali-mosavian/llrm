//! fib -O2 (60 B against 52 B at -O1, SDCC 54 B): `bench_fib` was inlined into `main`, 8 B more for a call's 27 T
//! in 2.5M: the estimate saw 2 B (the argument stays for the call the body makes; the sign extension has its
//! moves). A growth the estimate puts under the call is also tried without, and the module price decides.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

#[test]
fn a_wrapper_that_adds_bytes_stays_a_call_at_o2() {
    let ir = include_str!("data/fib_o2.ll");
    let asm = text(ir, "main", 0, &Request::new(Level::O2)).expect("compiles");
    assert!(asm.contains("call _bench_fib"), "bench_fib was inlined into main:\n{asm}");
}
