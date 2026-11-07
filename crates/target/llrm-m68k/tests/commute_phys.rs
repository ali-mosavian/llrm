//! `a * b + a / b + a % b` ended `add.l %d0,%d2; move.l %d2,%d0` before the
//! return: the add was tied to the sum that lives in `d2` and its result had
//! to be copied into `d0`, where the multiply had just left the other
//! operand. The pin's `SrcRegMap`/`DstRegMap` rule ties the result to the
//! source that arrives in the register it leaves in (muldiv 68000 -O2: 56 B,
//! gcc 54).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const MULDIV: &str = include_str!("../bench/library/muldiv.ll");

#[test]
fn a_sum_returned_in_the_register_its_operand_arrives_in_needs_no_copy() {
    let out = assembly(MULDIV, cpu_of("68000").unwrap(), Level::O2).expect("compiles");
    assert!(!out.contains("move.l\t%d2, %d0"), "the sum is copied into the return register:\n{out}");
    assert!(out.contains("add.l\t%d2, %d0"), "the sum is not added into the return register:\n{out}");
}
