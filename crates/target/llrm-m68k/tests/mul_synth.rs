//! `x * 1000` was `moveq #125; lsl.l #3; muls.l` at -O2 on the 68020 and a
//! `jsr __mulsi3` on the 68000, where gcc has a chain of shifts and adds: the
//! pin's combine decomposes `(2^n +- 1) << k` only, and 1000 is `125 << 3`
//! (div_constants_long 68020 -O2: 34668 clocks of `muls.l` against gcc's
//! 21060). DEVIATE G2: the cheapest chain by the m68k tables.

use llrm_m68k::driver::{Level, assembly, cpu_of};

fn mul_by(c: i32) -> String {
    format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\ndefine i32 @f(i32 %x) {{\n  %r = mul i32 %x, {c}\n  ret i32 %r\n}}\n"
    )
}

fn compile(cpu: &str, level: Level, c: i32) -> String {
    assembly(&mul_by(c), cpu_of(cpu).unwrap(), level).expect("compiles")
}

#[test]
fn a_constant_that_is_not_near_a_power_of_two_is_a_chain_where_clocks_are_priced() {
    for c in [1000, 100, -1000, 641, 31 * 5 * 8] {
        let out = compile("68020", Level::O2, c);
        assert!(!out.contains("muls.l") && !out.contains("__mulsi3"), "68020 -O2 x*{c}: a multiply:\n{out}");
    }
}

#[test]
fn the_68000_never_calls_the_library_for_a_short_chain() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let out = compile("68000", level, 1000);
        assert!(!out.contains("__mulsi3"), "68000 {level:?} x*1000: a call:\n{out}");
    }
}

#[test]
fn a_size_build_keeps_the_multiply_where_the_chain_is_longer() {
    let out = compile("68020", Level::Os, 1000);
    assert!(out.contains("muls.l"), "68020 -Os x*1000: a chain longer than 4 B:\n{out}");
}
