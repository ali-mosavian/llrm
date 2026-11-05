//! LFTR rewrote the exit compare of `do { s += i; } while (++i < n)` to a
//! counter and guarded it with `smax(n, 1)`: 8 B for `cmpi`, `bgt` and
//! `moveq` that a 68020's `cmp.l` with a memory limit does not repay (sum -O2
//! 36 B against gcc's 18, arrsum 32 against 22). The 68000 keeps the guard:
//! there the memory compare is 10 clk per trip dearer.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const SUM: &str = include_str!("../bench/kernels/sum.ll");

#[test]
fn a_68020_does_not_guard_a_counter_exit_with_smax() {
    let out = assembly(SUM, cpu_of("68020").unwrap(), Level::O2).expect("compiles");
    assert!(!out.contains("cmpi.l\t#1,"), "68020: the smax guard:\n{out}");
}

#[test]
fn a_68000_keeps_the_pins_counter_exit() {
    let out = assembly(SUM, cpu_of("68000").unwrap(), Level::O2).expect("compiles");
    assert!(out.contains("cmpi.l\t#1,"), "68000: no guard, the pin's exit changed:\n{out}");
}
