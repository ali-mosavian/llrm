//! A long shifted left by one.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// The instructions of `@name` in the printed assembly.
fn instructions(asm: &str, name: &str) -> usize {
    let start = asm.find(&format!("{name}::")).expect("function present");
    asm[start..]
        .lines()
        .skip(1)
        .take_while(|line| !line.trim_end().ends_with("::"))
        .filter(|line| line.starts_with('\t') && !line.trim_start().starts_with('.'))
        .count()
}

/// `shl i32 %n, 1` took 27 instructions: `srl l` seven times for bit 15 of the
/// low word, two `or`s of lanes, `sla c; rl b`, `sla e; rl d` and moves, then
/// (as `x + x`) a compare and a branch for the carry. It is `add hl, hl; rl e; rl d`
/// (long_double_in_place.rs: 10 instructions before the doubling was one fused instruction).
#[test]
fn a_long_shifted_left_by_one_is_an_add_with_carry() {
    let asm = text(include_str!("kernels/ir/shl_long_by_one.ll"), "f", 0, &Request::new(Level::O2)).expect("compiles");
    let count = instructions(&asm, "_f");
    assert!(count <= 4 && !asm.contains("jr "), "{count} instructions:\n{asm}");
}

/// The target combine ran on every target node and read operand 1 of a node with
/// one: indexing a global array panicked ("the len is 1 but the index is 1"),
/// which every program with a table hit (shift_int32_variable, shl_long_by_one).
#[test]
fn a_global_indexed_by_a_variable_compiles() {
    let asm = text(include_str!("kernels/ir/index_wrapped_global.ll"), "f", 0, &Request::new(Level::O2)).expect("compiles");
    assert!(asm.contains("_f::"), "{asm}");
}

/// The i16 `shl x, 1 -> add x, x` lowering that came with the long's funnel
/// stayed after the combine replaced it: every multiply by a constant that
/// decomposes into shifts (struct_odd_sizes `three`) went from 110 to 114
/// instructions at -Os (123 to 127 through the llc binary), 17 of 74 programs larger (11-07, #7847).
#[test]
#[ignore = "Z80-FUNNEL-RED until Z80S1 (2026-12-15): #8867 reorders the Source schedule, a pair spills"]
fn a_multiply_by_constants_is_no_larger_for_the_long_shift_combine() {
    let asm = text(include_str!("kernels/ir/mul_by_constants_sum.ll"), "three", 0, &Request::new(Level::Os)).expect("compiles");
    let count = instructions(&asm, "_three");
    assert!(count <= 110, "{count} instructions:\n{asm}");
}
