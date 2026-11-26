//! `llvm.rint` on a soft-float target is the C library's `rint`
//! (`SoftenFloatRes_Unary`, RTLIB::RINT_*): a program using it was refused
//! "Unsupported FRint, soften: the target names no routine".

mod common;

use llrm_6502::driver::{Level, Request, assembly};

#[test]
fn rint_of_a_float_and_a_double_calls_the_c_routine() {
    let ir = format!(
        "target datalayout = \"{}\"\n{}",
        common::pipeline::datalayout(),
        "declare float @llvm.rint.f32(float)\ndeclare double @llvm.rint.f64(double)\n\
         define float @rf(float %x) {\n  %r = call float @llvm.rint.f32(float %x)\n  ret float %r\n}\n\
         define double @rd(double %x) {\n  %r = call double @llvm.rint.f64(double %x)\n  ret double %r\n}\n"
    );
    for level in [Level::O1, Level::O2, Level::Os] {
        let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|why| panic!("{level:?}: {why}"));
        assert!(text.contains("rintf") && text.contains("rint\n"), "{level:?}: no call:\n{text}");
    }
}
