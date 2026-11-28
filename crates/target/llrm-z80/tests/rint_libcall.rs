//! `llvm.rint` on a soft-float target is the C library's `rint`
//! (`SoftenFloatRes_Unary`, RTLIB::RINT_*): a program using it was refused
//! "Unsupported FRint, soften: the target names no routine".

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8";

#[test]
fn rint_of_a_float_and_a_double_calls_the_c_routine() {
    let ir = format!(
        "target datalayout = \"{DATALAYOUT}\"\n\
         declare float @llvm.rint.f32(float)\ndeclare double @llvm.rint.f64(double)\n\
         define float @rf(float %x) {{\n  %r = call float @llvm.rint.f32(float %x)\n  ret float %r\n}}\n\
         define double @rd(double %x) {{\n  %r = call double @llvm.rint.f64(double %x)\n  ret double %r\n}}\n"
    );
    // SDCC's C symbols take an underscore: a call of plain `rint` did not link
    // to the program's `_rint`.
    for (name, callee) in [("rf", "_rintf"), ("rd", "_rint")] {
        let asm = text(&ir, name, 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{name}: {why}"));
        let called = asm
            .lines()
            .any(
                |line| matches!(
                    line.trim().split_once(' '),
                    Some(("call" | "jp", to)) if to.trim() == callee
                ),
            );
        assert!(called, "{name}: no call to {callee}:\n{asm}");
    }
}
