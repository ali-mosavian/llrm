//! `x * 9` was `jsr __mulsi3` on the 68000 (`muls.l` on the 68020) because the
//! m68k left the pin's `decomposeMulByConstant` at its default, false:
//! mul_constants -Os 68000 whole program +554 B against the pin's clang. `2^n
//! +- 1` times a power of two is shifts and one add on every CPU.

use llrm_m68k::driver::{Level, assembly, cpu_of};

fn mul_by(c: u32) -> String {
    format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\ndefine i32 @f(i32 %x) {{\n  %r = mul i32 %x, {c}\n  ret i32 %r\n}}\n"
    )
}

#[test]
fn a_product_by_a_near_power_of_two_is_shifts_and_an_add_not_a_multiply() {
    for (cpu, multiply) in [("68000", "__mulsi3"), ("68020", "muls.l")] {
        for level in [Level::O2, Level::Os] {
            for c in [3, 5, 9, 10, 127, 255] {
                let out = assembly(&mul_by(c), cpu_of(cpu).unwrap(), level).expect("compiles");
                assert!(!out.contains(multiply), "{cpu} x*{c}: a multiply:\n{out}");
            }
        }
    }
}
