//! A float operation panicked the compiler (`RetCC_M68k_C names a register`):
//! the shared legalizer left `f32` and `f64` legal. A target without an FPU
//! calls the runtime, as the pin does at every CPU: `fadd double` is
//! `jsr __adddf3` with the two doubles as four words and the result in
//! `d0:d1`.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

/// One function per class of float operation, and the routine it calls.
const CASES: &[(&str, &str, &str)] = &[
    ("define double @f(double %a, double %b) { %r = fadd double %a, %b ret double %r }", "f", "__adddf3"),
    ("define double @f(double %a, double %b) { %r = fsub double %a, %b ret double %r }", "f", "__subdf3"),
    ("define float @f(float %a, float %b) { %r = fmul float %a, %b ret float %r }", "f", "__mulsf3"),
    ("define double @f(double %a, double %b) { %r = fdiv double %a, %b ret double %r }", "f", "__divdf3"),
    ("define double @f(float %a) { %r = fpext float %a to double ret double %r }", "f", "__extendsfdf2"),
    ("define float @f(double %a) { %r = fptrunc double %a to float ret float %r }", "f", "__truncdfsf2"),
    ("define float @f(i32 %a) { %r = sitofp i32 %a to float ret float %r }", "f", "__floatsisf"),
    ("define double @f(i32 %a) { %r = uitofp i32 %a to double ret double %r }", "f", "__floatunsidf"),
    ("define double @f(i16 %a) { %r = sitofp i16 %a to double ret double %r }", "f", "__floatsidf"),
    ("define i32 @f(double %a) { %r = fptosi double %a to i32 ret i32 %r }", "f", "__fixdfsi"),
    ("define i32 @f(float %a) { %r = fptoui float %a to i32 ret i32 %r }", "f", "__fixunssfsi"),
    (
        "define i32 @f(double %a, double %b) { %c = fcmp olt double %a, %b %r = zext i1 %c to i32 ret i32 %r }",
        "f",
        "__ltdf2",
    ),
    (
        "define i32 @f(float %a, float %b) { %c = fcmp ugt float %a, %b %r = zext i1 %c to i32 ret i32 %r }",
        "f",
        "__lesf2",
    ),
    ("define double @f(double %a) { %r = fadd double %a, 1.5 ret double %r }", "f", "__adddf3"),
];

fn text(
    ir: &str,
    name: &str,
    cpu: Cpu,
) -> Result<String, String> {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses");
    text_with(&module, name, cpu, Options::default())
}

#[test]
fn every_float_operation_is_a_call_of_the_runtime() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        for (ir, name, routine) in CASES {
            let out = text(ir, name, cpu).unwrap_or_else(|error| panic!("{cpu:?} {ir}: {error}"));
            assert!(out.contains(&format!("jsr\t{routine}")), "{cpu:?} {ir}: no call of {routine}:\n{out}");
        }
    }
}

/// A double argument is two words, the high one first, and a double result is
/// the high word in `d0`.
#[test]
fn a_double_is_its_high_word_first() {
    let out = text("define double @f(double %a) { ret double %a }", "f", Cpu::M68020).expect("compiles");
    assert!(out.contains("move.l\t(4,%sp), %d0") && out.contains("move.l\t(8,%sp), %d1"), "{out}");
}
