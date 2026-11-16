//! At -Os below the 68020 a byte or word divide by a constant stayed the
//! multiply-high sequence (`muls.w`, `swap`, shifts: 12 instructions for
//! `short / 10`) while one `divs.w` is 4 B. The word divide is an instruction
//! at every CPU, the long one a call before the 68020.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const LAYOUT: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

fn asm(
    op: &str,
    ty: &str,
    cpu: &str,
    level: Level,
) -> String {
    let ir = format!("{LAYOUT}define {ty} @f({ty} %x) {{\n  %r = {op} {ty} %x, 10\n  ret {ty} %r\n}}");
    assembly(&ir, cpu_of(cpu).unwrap(), level).expect("compiles")
}

#[test]
fn a_word_divide_by_a_constant_is_one_divide_in_a_size_build() {
    for cpu in ["68000", "68010"] {
        for (op, divide) in [("sdiv", "divs\t"), ("udiv", "divu\t")] {
            let out = asm(op, "i16", cpu, Level::Os);
            assert!(out.contains(divide) && !out.contains("mul"), "{cpu} {op}:\n{out}");
        }
    }
}

#[test]
fn a_long_divide_before_the_68020_stays_a_call_or_the_multiply_sequence() {
    let out = asm("sdiv", "i32", "68000", Level::Os);
    assert!(out.contains("__divsi3") && !out.contains("divs\t"), "{out}");
}

#[test]
fn a_speed_build_keeps_the_multiply_for_a_word_divide() {
    let out = asm("sdiv", "i16", "68000", Level::O2);
    assert!(out.contains("muls\t") && !out.contains("divs\t"), "{out}");
}
