//! A 64-bit multiply of two variables in a function built for size was ~100 B
//! of 16-bit partial products and `__mulsi3` calls at the 68000 and 68010
//! (int64_divmod_edges -Os: `pair` 402 B against gcc's 236, `main` 728 against
//! 410). One `__muldi3` call is the size routine, as `__muldi3` is gcc's.
//! Speed levels keep the partial products.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
define i64 @f(i64 %a, i64 %b) {\n  %r = mul i64 %a, %b\n  ret i64 %r\n}\n";

#[test]
fn a_size_built_variable_i64_multiply_is_one_muldi3_call() {
    for cpu in ["68000", "68010"] {
        let text = assembly(IR, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        assert!(text.contains("__muldi3"), "{cpu}: no call:\n{text}");
        assert!(!text.contains("mulu"), "{cpu}: partial products:\n{text}");
    }
}

#[test]
fn a_speed_built_variable_i64_multiply_keeps_the_partial_products() {
    for cpu in ["68000", "68010"] {
        let text = assembly(IR, cpu_of(cpu).unwrap(), Level::O2).expect("compiles");
        assert!(!text.contains("__muldi3"), "{cpu}: a call at speed:\n{text}");
    }
}
