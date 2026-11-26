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

/// A whole program that defines `rint` and `rintf` and calls `llvm.rint`: the
/// call appears in the DAG, after Internalize and GlobalDCE had deleted both
/// definitions as unreferenced, so the link failed on `rint rintf`. A routine
/// a libcall may name is kept, and the call is spelled as the platform spells
/// the definition.
#[test]
fn whole_program_keeps_the_definitions_of_the_routines_a_libcall_names() {
    let ir = format!(
        "target datalayout = \"{}\"\n{}",
        common::pipeline::datalayout(),
        "declare float @llvm.rint.f32(float)\ndeclare double @llvm.rint.f64(double)\n\
         define float @rintf(float %x) {\n  ret float %x\n}\n\
         define double @rint(double %x) {\n  ret double %x\n}\n\
         define float @unused(float %x) {\n  ret float %x\n}\n\
         define double @main(double %x) {\n  %f = fptrunc double %x to float\n  \
         %a = call float @llvm.rint.f32(float %f)\n  %b = fpext float %a to double\n  \
         %r = call double @llvm.rint.f64(double %b)\n  ret double %r\n}\n"
    );
    for (platform, prefix) in [(llrm_6502::platform::SIM65, ""), (SIM65_CC65, "_")] {
        let mut request = Request::new(Level::O2);
        request.platform = llrm_6502::platform::Platform::parse(platform).expect("parses");
        request.passes = vec!["-fwhole-program".to_owned()];
        let text = assembly(&ir, &request).unwrap_or_else(|why| panic!("{prefix:?}: {why}"));
        for name in ["rint", "rintf"] {
            let symbol = format!("{prefix}{name}");
            assert!(text.contains(&format!("jsr {symbol}")), "{prefix:?}: no call of {symbol}:\n{text}");
            assert!(text.contains(&format!("{symbol}:")), "{prefix:?}: {symbol} is not defined:\n{text}");
        }
        assert!(!text.contains(&format!("{prefix}unused")), "{prefix:?}: the unreferenced routine stays:\n{text}");
    }
}

const SIM65_CC65: &str = include_str!("../src/machines/sim65-cc65.toml");
