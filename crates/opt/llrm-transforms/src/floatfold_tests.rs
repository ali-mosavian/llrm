//! Port of `tests/test_floatfold.py`, each body now MIR text run by
//! llrm-mir's interpreter before and after.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::module::Module;
use llrm_mir::passes::Outer;

use super::{FloatFold, folded};
use crate::dead::dead;
use crate::testing::{managed, parsed, printed, results};

/// @f folded; whether anything changed.
fn fold(module: &mut Module) -> bool {
    let (layout, outer) = (layout(module), Outer::of(module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    folded(context, &layout, function, &outer, &llrm_analysis::consts::Calls::default())
}

/// `body` folded, and then its dead work gone when `clean`: its text, and
/// that it computes what it did.
fn folded_text(
    body: &str,
    inputs: &[&[i128]],
    clean: bool,
) -> String {
    let mut module = parsed(&format!("{DOS}{body}"));
    let before = results(&module, inputs);
    fold(&mut module);
    if clean {
        let callees = llrm_mir::memory::callees(&module);
        let (context, function) = module.function_mut("f").expect("@f");
        dead(context, &callees, function);
    }
    assert_eq!(results(&module, inputs), before, "{body}");
    printed(&module)
}

/// Only an exact value is stored as its bits.
#[test]
fn test_storage_requires_exact_bits() {
    for (ty, source, expected) in [
        ("float", "sitofp i32 144 to double", Some("1.440000e+02")),
        ("float", "sitofp i32 -6 to double", Some("-6.000000e+00")),
        ("float", "sitofp i32 16777217 to double", None),
        ("float", "fdiv double 1.0, 3.0", None),
        ("double", "sitofp i32 12 to double", Some("1.200000e+01")),
        ("double", "sitofp i32 -6 to double", Some("-6.000000e+00")),
        ("double", "sitofp i64 9007199254740993 to double", None),
        ("double", "fdiv double 1.0, 3.0", None),
    ] {
        let narrowed = if ty == "float" { "fptrunc double %x to float" } else { "fadd double %x, 0.0" };
        let text = folded_text(
            &format!(
                "@g = global {ty} 0.0

define {ty} @f() {{
b0:
  %x = {source}
  %y = {narrowed}
  store {ty} %y, ptr @g
  %r = load {ty}, ptr @g
  ret {ty} %r
}}
"
            ),
            &[&[]],
            false,
        );
        match expected {
            Some(constant) => assert!(
                text.contains(&format!("store {ty} {constant}, ptr @g"))
                    && text.contains(&format!("ret {ty} {constant}")),
                "{text}"
            ),
            None => assert!(
                text.contains(&format!("store {ty} %y, ptr @g")) && text.contains(&format!("ret {ty} %r")),
                "{text}"
            ),
        }
    }
}

/// An exact conversion's integer is its readers' number; its float work
/// and its load go as dead.
#[test]
fn test_an_exact_conversion_reads_as_its_number() {
    let body = "@g = global float 0.0

define i32 @f() {
b0:
  store float 1.440000e+02, ptr @g
  %x = load float, ptr @g
  %i = fptosi float %x to i32
  ret i32 %i
}
";
    assert_eq!(
        folded_text(body, &[&[]], true),
        format!(
            "{DOS}@g = global float 0.000000e+00

define i32 @f() {{
b0:
  store float 1.440000e+02, ptr @g
  ret i32 144
}}
"
        )
    );
}

/// What the old pair refused: an unknown value, or memory it may not read.
#[test]
fn test_an_unknown_conversion_stays() {
    for body in [
        "define i32 @f(i16 %n) {
b0:
  %x = sitofp i16 %n to float
  %y = fmul float %x, 0.5
  %i = fptosi float %y to i32
  ret i32 %i
}
",
        "@g = global float 0.0

define i32 @f(i16 %n) {
b0:
  store volatile float 1.440000e+02, ptr @g
  %x = load volatile float, ptr @g
  %i = fptosi float %x to i32
  ret i32 %i
}
",
    ] {
        let mut module = parsed(&format!("{DOS}{body}"));
        let before = printed(&module);
        assert!(!fold(&mut module), "{body}");
        assert_eq!(printed(&module), before);
    }
}

/// FPDEEP computed d=12 and e=6 after proving both exact: none of it runs.
#[test]
fn test_exact_double_stores_do_not_execute_floating_arithmetic() {
    let body = "@m = global [16 x i8] zeroinitializer

define double @f() {
b0:
  %a = sitofp i16 3 to double
  %b = fmul double %a, 4.0
  store double %b, ptr @m
  %c = fdiv double %b, 2.0
  store double %c, ptr getelementptr (i8, ptr @m, i16 8)
  %d = load double, ptr @m
  %e = load double, ptr getelementptr (i8, ptr @m, i16 8)
  %s = fsub double %d, %e
  ret double %s
}
";
    assert_eq!(
        folded_text(body, &[&[]], true),
        format!(
            "{DOS}@m = global [16 x i8] zeroinitializer

define double @f() {{
b0:
  store double 1.200000e+01, ptr @m
  store double 6.000000e+00, ptr getelementptr (i8, ptr @m, i16 8)
  ret double 6.000000e+00
}}
"
        )
    );
}

/// NaN, infinity, an inexact result or a cancellation's zero stay; a
/// signed zero an exact product makes folds with its sign.
#[test]
fn test_what_may_not_be_exact_stays() {
    for (operation, expected) in [
        ("fdiv double 1.0, 0.0", None),
        ("fdiv double 0.0, 0.0", None),
        ("fdiv double 1.0, 3.0", None),
        ("fadd double 0x7FF0000000000000, 1.0", None),
        ("fmul double 0x0010000000000000, 0.5", None),
        ("fsub double 1.5, 1.5", None),
        ("fsub nsz double 1.5, 1.5", Some("0.000000e+00")),
        ("fmul double -0.0, 1.0", Some("-0.000000e+00")),
        ("fneg double 0.0", Some("-0.000000e+00")),
        ("fadd double -0.0, -0.0", Some("-0.000000e+00")),
    ] {
        let body = format!(
            "define double @f() {{
b0:
  %x = {operation}
  ret double %x
}}
"
        );
        let text = folded_text(&body, &[&[]], false);
        let returned = expected.unwrap_or("%x");
        assert!(text.contains(&format!("ret double {returned}")), "{operation}: {text}");
    }
}

#[test]
fn test_the_pass_runs_under_the_manager() {
    let mut module = parsed(&format!(
        "{DOS}define float @f() {{
b0:
  %x = call float @llvm.sqrt.f32(float 2.25)
  %y = call float @llvm.fabs.f32(float -2.0)
  %z = fmul float %x, %y
  ret float %z
}}

declare float @llvm.sqrt.f32(float)

declare float @llvm.fabs.f32(float)
"
    ));
    let before = results(&module, &[&[]]);
    assert!(managed(&mut module, FloatFold).contains("ret float 3.000000e+00"));
    assert_eq!(results(&module, &[&[]]), before);
}

/// @h writes nothing, which only the module's summaries say: the float
/// @g holds survives the call.
pub(crate) const ACROSS_A_CALL: &str = "@g = global float 0.0

define void @h() {
b0:
  ret void
}

define float @f() {
b0:
  store float 1.5, ptr @g
  call void @h()
  %x = load float, ptr @g
  %y = fadd float %x, %x
  ret float %y
}
";

/// `module` through `pass`, `Summaries` required, printed.
pub(crate) fn summarized(
    module: &mut Module,
    pass: impl llrm_mir::passes::FunctionPass + 'static,
) -> String {
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.require::<llrm_analysis::manager::Summaries>();
    manager.add(pass);
    manager.run_module(module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    printed(module)
}

#[test]
fn test_a_float_cell_is_kept_across_a_call_that_cannot_write_it() {
    let mut module = parsed(&format!("{DOS}{ACROSS_A_CALL}"));
    let before = results(&module, &[&[]]);
    assert!(summarized(&mut module, FloatFold).contains("ret float 3.000000e+00"));
    assert_eq!(results(&module, &[&[]]), before);
}

/// `body` over `double %x`, through the compilers' pipeline, as text. The
/// function is internal and called from @main, so it is neither dropped nor
/// inlined away.
fn through_the_pipeline(body: &str) -> String {
    use llrm_mir::program::Program;
    let text = format!(
        "{DOS}define internal double @f(double %x) {{\nb1:\n{body}\n}}\n\ndefine double @main(double %x) {{\nb0:\n  %r = call double @f(double %x)\n  ret double %r\n}}\n"
    );
    let mut module = parsed(&text);
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        program.exports.entries.insert("main".to_owned());
        crate::pipeline::applied(program, &crate::pipeline::Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
    printed(&module)
}

/// What the language lets a floating operation lose, each rule only under its flag, in
/// the pipeline the compilers run: a division by a constant a multiply by its reciprocal
/// (`arcp`), two constants of a chain one (`reassoc`), a zero's sign unobserved (`nsz`),
/// no NaN or infinity (`nnan`, `ninf`). Without the flag the instruction stays:
/// `x / 3.0` differs from `x * (1/3.0)` in its last bit, `x * 0.0` is NaN for a NaN,
/// `x + 0.0` is +0.0 for -0.0. The rules were in `instcombine`, which no compiler runs.
#[test]
fn test_floating_flags_license_the_folds_and_their_absence_keeps_the_operation() {
    let said = |body: &str| through_the_pipeline(body);
    let stays = |body: &str, kept: &str| {
        let text = said(body);
        assert!(text.contains(kept), "{kept} gone\n{text}");
    };
    // arcp
    let arcp = said("  %r = fdiv arcp double %x, 3.0\n  ret double %r");
    assert!(
        arcp.contains("fmul arcp double %0, 0x3FD5555555555555")
            || arcp.contains("fmul arcp double %x, 0x3FD5555555555555"),
        "{arcp}"
    );
    stays("  %r = fdiv double %x, 3.0\n  ret double %r", "fdiv double");
    // reassoc, both operations
    let chain = |first: &str, second: &str| {
        format!("  %a = fadd {first} double %x, 1.0\n  %r = fadd {second} double %a, 2.0\n  ret double %r")
    };
    let both = said(&chain("reassoc", "reassoc"));
    assert!(
        both.contains("fadd reassoc double") && both.contains("3.000000e+00") && both.matches("fadd").count() == 1,
        "{both}"
    );
    assert_eq!(said(&chain("reassoc", "")).matches("fadd").count(), 2);
    assert_eq!(said(&chain("", "reassoc")).matches("fadd").count(), 2);
    // nsz
    assert!(!said("  %r = fadd nsz double %x, 0.0\n  ret double %r").contains("fadd"));
    stays("  %r = fadd double %x, 0.0\n  ret double %r", "fadd double");
    assert!(!said("  %r = fadd double %x, -0.0\n  ret double %r").contains("fadd"));
    // nnan and nsz make x * 0 a zero; each alone does not
    assert!(!said("  %r = fmul nnan nsz double %x, 0.0\n  ret double %r").contains("fmul"));
    stays("  %r = fmul nnan double %x, 0.0\n  ret double %r", "fmul");
    stays("  %r = fmul nsz double %x, 0.0\n  ret double %r", "fmul");
    // nnan and ninf make x - x a zero and x / x a one
    assert!(!said("  %r = fsub nnan ninf double %x, %x\n  ret double %r").contains("fsub"));
    assert!(!said("  %r = fdiv nnan ninf double %x, %x\n  ret double %r").contains("fdiv"));
    stays("  %r = fsub nnan double %x, %x\n  ret double %r", "fsub");
    // a multiply by one needs no flag
    assert!(!said("  %r = fmul double 1.0, %x\n  ret double %r").contains("fmul"));
}
