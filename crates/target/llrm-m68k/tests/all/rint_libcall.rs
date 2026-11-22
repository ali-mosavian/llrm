//! `llvm.rint` on a soft-float target is the C library's `rint`
//! (`SoftenFloatRes_Unary`, RTLIB::RINT_*): QuickrBasic `fpmath.c`, `double.c`,
//! `exec.c` and `operands.c` were refused "Unsupported FRint, soften a float
//! result".

use llrm_m68k::driver::{Level, assembly, cpu_of};

#[test]
fn rint_of_a_float_and_a_double_calls_the_c_routine() {
    let source = include_str!("../data/rint_libcall.ll");
    for cpu in ["68000", "68020"] {
        let text = assembly(source, cpu_of(cpu).unwrap(), Level::O2).unwrap_or_else(|why| panic!("{cpu}: {why}"));
        assert!(text.contains("jsr\trintf") && text.contains("jsr\trint\n"), "{cpu}: no call:\n{text}");
    }
}
