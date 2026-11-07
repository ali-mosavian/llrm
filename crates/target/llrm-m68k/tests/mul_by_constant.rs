//! `x * 7` and `x * 31` called `__mulsi3` (`pea 7; move.l x,-(%sp); jsr; addq.l
//! #8,%sp`, 14 B) where the pin's `decomposeMulByConstant` makes them `lsl.l
//! #3; sub.l` (6 B): the hook was not translated (int64_division -Os: 14 calls,
//! mulconst_i64 45, overflow_builtins 42, gcc 5 and 3).

use llrm_m68k::driver::{Level, assembly, cpu_of};

fn text(
    factor: i64,
    cpu: &str,
) -> String {
    let ir = format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @f(i32 %x) {{
  %r = mul i32 %x, {factor}
  ret i32 %r
}}
"
    );
    assembly(&ir, cpu_of(cpu).unwrap(), Level::O2).expect("compiles")
}

#[test]
fn a_32_bit_multiply_by_two_to_the_n_minus_one_is_a_shift_and_a_subtract() {
    for cpu in ["68000", "68020"] {
        for (factor, shift) in [(7, 3), (31, 5)] {
            let out = text(factor, cpu);
            assert!(!out.contains("__mulsi3") && !out.contains("muls"), "{cpu} x*{factor}:\n{out}");
            assert!(out.contains(&format!("lsl.l\t#{shift}")), "{cpu} x*{factor}:\n{out}");
        }
    }
}

/// The pin decomposes only `2^n +- 1` (times a power of two): 100 stays a
/// multiply.
#[test]
fn another_constant_stays_a_multiply() {
    assert!(text(100, "68000").contains("__mulsi3"));
    assert!(text(100, "68020").contains("muls.l"));
}
