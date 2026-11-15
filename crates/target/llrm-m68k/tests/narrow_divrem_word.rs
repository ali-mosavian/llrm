//! `short / 10` at -O1 (the pin's O1 has no correlated-propagation to narrow
//! it) was `__divsi3`: ~600 clk on the 68000 (div_constants_short O1, +59%
//! clocks). Operands that fit 16 bits divide at 16 bits on the CPUs whose long
//! divide is a call.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const LAYOUT: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

fn asm(
    body: &str,
    cpu: &str,
    level: Level,
) -> String {
    // `x` and `y` are `(short)v` widened again: 17 sign bits, no `sext` of an argument to see through
    let ir = format!(
        "{LAYOUT}define i32 @f(i32 %v, i32 %w) {{\n  %t = trunc i32 %v to i16\n  %x = sext i16 %t to i32\n  %u = trunc i32 %w to i16\n  %y = sext i16 %u to i32\n{body}\n}}"
    );
    assembly(&ir, cpu_of(cpu).unwrap(), level).expect("compiles")
}

#[test]
fn a_short_divided_by_a_constant_is_not_a_call_at_o1() {
    for op in ["sdiv", "srem"] {
        let out = asm(&format!("  %r = {op} i32 %x, 10\n  ret i32 %r"), "68000", Level::O1);
        assert!(!out.contains("__"), "{op}:\n{out}");
    }
}

#[test]
fn a_variable_divisor_that_may_be_minus_one_stays_a_call() {
    let out = asm("  %r = sdiv i32 %x, %y\n  ret i32 %r", "68000", Level::O1);
    assert!(out.contains("__divsi3"), "{out}");
}

#[test]
fn the_68020_has_no_call_either_and_keeps_its_long_multiply() {
    let out = asm("  %r = sdiv i32 %x, 10\n  ret i32 %r", "68020", Level::O1);
    assert!(!out.contains("__") && out.contains("muls.l"), "{out}");
}
