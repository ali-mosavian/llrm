//! `IndVarSimplify` replaced the loop of `s += i` (bench/kernels/sum, 16 B in
//! gcc) with its closed form, a multiply in `i33` the pin's base class prices
//! at one operation: 268 B of `__mulsi3` calls. A multiply past 32 bits is a
//! library call here and costs as one.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const SUM: &str = include_str!("../bench/kernels/sum.ll");

#[test]
fn a_sum_loop_stays_a_loop_instead_of_a_wide_multiply() {
    for cpu in ["68000", "68020"] {
        let out = assembly(SUM, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        assert!(!out.contains("mulsi3") && !out.contains("muldi3"), "{cpu}: the closed form:\n{out}");
        assert!(out.contains("add.l"), "{cpu}: no loop:\n{out}");
    }
}
