//! `bench_sieve` (inlining cost 105) stayed a call under the flat threshold of
//! 50 the m68k carried, though the pin (225) inlines it: `limit` was then no
//! constant in the loop and the `i * i` multiply ran ahead of its `i < 32`
//! guard, a `__mulsi3` per prime (sieve 68000 O1 289,112 clocks against gcc
//! 189,350).

use llrm_m68k::driver::{Level, assembly, cpu_of};

/// clang -O0 IR of bench/sieve, `noinline` dropped.
fn ir() -> String {
    include_str!("fixtures/sieve_o0.ll").replace("noinline ", "")
}

#[test]
fn main_inlines_a_callee_the_pin_inlines() {
    let ir = ir();
    for cpu in ["68000", "68020"] {
        for level in [Level::O2, Level::O3] {
            let out = assembly(&ir, cpu_of(cpu).unwrap(), level).expect("compiles");
            let main = out.split("main:").nth(1).expect("main");
            assert!(!main.contains("bench_sieve"), "{cpu} {level:?} kept the call:\n{out}");
        }
    }
}
