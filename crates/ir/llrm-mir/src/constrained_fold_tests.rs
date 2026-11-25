use crate::context::ConstantKind;
use crate::module::{GlobalKind, Module, Operand};
use crate::parse;

const DECLARES: &str = "declare fp128 @llvm.experimental.constrained.fpext.f128.f32(float, metadata) strictfp
declare double @llvm.experimental.constrained.fpext.f64.f32(float, metadata) strictfp
declare float @llvm.experimental.constrained.fptrunc.f32.f128(fp128, metadata, metadata) strictfp
";

/// The constant `simplify_instruction` makes of `%r`, as its kind; None where
/// the call stays.
fn folded(body: &str) -> Option<ConstantKind> {
    let text = format!("{DECLARES}define void @f() strictfp {{\nb0:\n{body}  ret void\n}}\n");
    let mut module: Module = parse::module(&text).unwrap_or_else(|why| panic!("{why:?}\n{text}"));
    let id = module.named("f").expect("@f");
    let GlobalKind::Function(function) = &module.globals[id.0 as usize].kind else { panic!("a function") };
    let function = function.clone();
    let inst = function
        .walk()
        .map(|(_, inst)| inst)
        .find(|&inst| function.instruction(inst).result.is_some_and(|r| function.value(r).name.as_deref() == Some("r")))
        .expect("%r");
    let found = crate::simplify::simplify_instruction_in(
        &mut module.context,
        &module.globals,
        &crate::datalayout::DataLayout::default(),
        None,
        &function,
        inst,
    )?;
    let Operand::Constant(id) = found else { return None };
    Some(module.context.get(id).kind.clone())
}

const STRICT: &str = "metadata !\"fpexcept.strict\"";
const DYNAMIC: &str = "metadata !\"round.dynamic\"";

/// QB pfpu.c `scale10`, arm64 Os, +24 B against gcc:
/// `constrained.fpext.f128.f32(float 3.2767e4)` stayed a call (and its
/// libcall-free expansion) although the widening raises no exception.
#[test]
fn a_widening_of_a_constant_folds_under_strict_exceptions() {
    let kind = folded(&format!(
        "  %r = call fp128 @llvm.experimental.constrained.fpext.f128.f32(float 3.276750e+04, {STRICT}) strictfp\n"
    ));
    assert!(matches!(kind, Some(ConstantKind::Float128(_))), "{kind:?}");
    let kind = folded(&format!(
        "  %r = call double @llvm.experimental.constrained.fpext.f64.f32(float 1.5, {STRICT}) strictfp\n"
    ));
    assert!(matches!(kind, Some(ConstantKind::Float(bits)) if f64::from_bits(bits) == 1.5), "{kind:?}");
}

/// A narrowing folds only where it is exact: 0.5 fits a float, 0.1 does not
/// (inexact status).
#[test]
fn a_narrowing_folds_only_where_it_is_exact() {
    let call = |bits: &str| {
        format!(
            "  %r = call float @llvm.experimental.constrained.fptrunc.f32.f128(fp128 {bits}, {DYNAMIC}, {STRICT}) strictfp\n"
        )
    };
    let half = folded(&call("0xL00000000000000003FFE000000000000"));
    assert!(matches!(half, Some(ConstantKind::Float(bits)) if f32::from_bits(bits as u32) == 0.5), "{half:?}");
    assert_eq!(folded(&call("0xL999999999999999A3FFB999999999999")), None);
}

/// A NaN keeps its call: the invalid status and the payload are the hardware's.
#[test]
fn a_nan_is_left_to_run() {
    let kind = folded(&format!(
        "  %r = call fp128 @llvm.experimental.constrained.fpext.f128.f32(float 0x7FF8000000000000, {STRICT}) strictfp\n"
    ));
    assert_eq!(kind, None);
}
