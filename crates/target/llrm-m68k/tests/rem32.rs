//! `x % y` on an `i32` was `jsr __modsi3` at every CPU, so a program linked a
//! ~478 B routine (17 corpus programs x 4 CPUs) that gcc avoids with the
//! 68020's `divsl.l`/`divul.l` (quotient and remainder in one divide).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
                  define i32 @s(i32 %a, i32 %b) {\n  %r = srem i32 %a, %b\n  ret i32 %r\n}\n\
                  define i32 @u(i32 %a, i32 %b) {\n  %r = urem i32 %a, %b\n  ret i32 %r\n}";

fn asm(
    cpu: &str,
    level: Level,
) -> String {
    assembly(IR, cpu_of(cpu).unwrap(), level).expect("compiles")
}

#[test]
fn the_remainder_is_inline_from_the_68020() {
    for cpu in ["68020", "68030", "68040", "68060"] {
        for level in [Level::O1, Level::O2, Level::Os] {
            let out = asm(cpu, level);
            assert!(
                !out.contains("__modsi3")
                    && !out.contains("__umodsi3")
                    && out.contains("divsl.l")
                    && out.contains("divul.l"),
                "{cpu} {level:?}:\n{out}"
            );
        }
    }
}

#[test]
fn the_remainder_is_still_a_call_below_the_68020() {
    for cpu in ["68000", "68010"] {
        let out = asm(cpu, Level::O2);
        assert!(out.contains("__modsi3") && out.contains("__umodsi3"), "{cpu}:\n{out}");
    }
}
