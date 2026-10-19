use crate::context::ConstantKind;
use crate::module::{Module, Operand};
use crate::parse;

/// What `simplify_instruction` makes of `%r` in `@f(float %x, float %y)`:
/// `%x` for a value, the float, `poison`, `NaN`; None where it finds nothing.
fn simplified(body: &str) -> Option<String> {
    let text = format!("define float @f(float %x, float %y) {{\nb0:\n{body}  ret float %x\n}}\n");
    let mut module: Module = parse::module(&text).unwrap_or_else(|why| panic!("{why:?}\n{text}"));
    let id = module.named("f").expect("@f");
    let crate::module::GlobalKind::Function(function) = &module.globals[id.0 as usize].kind else {
        panic!("a function")
    };
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
    Some(match found {
        Operand::Value(value) => format!("%{}", function.value(value).name.clone().unwrap_or_default()),
        Operand::Constant(id) => match module.context.get(id).kind {
            ConstantKind::Poison => "poison".to_owned(),
            ConstantKind::Null | ConstantKind::Zero => "0.0".to_owned(),
            ConstantKind::Float(bits) => format!("{:?}", f32::from_bits(bits as u32)),
            ref other => format!("{other:?}"),
        },
        Operand::Block(_) => unreachable!("an instruction is never a block"),
    })
}

fn check(
    body: &str,
    expected: Option<&str>,
) {
    assert_eq!(simplified(body).as_deref(), expected, "{body}");
}

/// `fmul float 1.0, %x` was left as it was (#2246).
#[test]
fn a_multiply_by_one_is_the_other_operand() {
    check("  %r = fmul float 1.0, %x\n", Some("%x"));
    check("  %r = fmul float %x, 1.0\n", Some("%x"));
    check("  %r = fmul float %x, %y\n", None);
    check("  %r = fdiv float %x, 1.0\n", Some("%x"));
}

#[test]
fn constants_fold_in_nearest_even() {
    check("  %r = fadd float 1.5, 2.5\n", Some("4.0"));
    check("  %r = fsub float 1.5, 2.5\n", Some("-1.0"));
    check("  %r = fmul float 3.0, 2.0\n", Some("6.0"));
    check("  %r = fdiv float 1.0, 4.0\n", Some("0.25"));
    check("  %r = frem float 5.5, 2.0\n", Some("1.5"));
    check("  %r = fdiv float 1.0, 3.0\n", Some("0.33333334"));
}

#[test]
fn additive_zeros_need_the_sign_of_zero() {
    check("  %r = fadd float %x, -0.0\n", Some("%x"));
    check("  %r = fadd float %x, 0.0\n", None);
    check("  %r = fadd nsz float %x, 0.0\n", Some("%x"));
    check("  %r = fsub float %x, 0.0\n", Some("%x"));
    check("  %r = fsub float %x, -0.0\n", None);
    check("  %r = fsub nsz float %x, -0.0\n", Some("%x"));
}

#[test]
fn nan_operands_propagate_and_undef_is_nan() {
    check("  %r = fadd float %x, 0x7FF8000000000000\n", Some("NaN"));
    check("  %r = fmul float undef, %x\n", Some("NaN"));
    check("  %r = fadd nnan float %x, undef\n", Some("poison"));
    check("  %r = fadd ninf float %x, 0x7FF0000000000000\n", Some("poison"));
    check("  %r = fadd float %x, poison\n", Some("poison"));
}

#[test]
fn multiply_by_zero_wants_nnan_and_nsz() {
    check("  %r = fmul float %x, 0.0\n", None);
    check("  %r = fmul nnan nsz float %x, 0.0\n", Some("0.0"));
    check("  %r = fmul nnan float %x, 0.0\n", None);
}

#[test]
fn negations_cancel_under_nnan() {
    check("  %n = fneg float %x\n  %r = fadd nnan float %x, %n\n", Some("0.0"));
    check("  %n = fneg float %x\n  %r = fadd float %x, %n\n", None);
    check("  %r = fsub nnan float %x, %x\n", Some("0.0"));
    check("  %r = fsub float %x, %x\n", None);
    check("  %n = fneg float %x\n  %r = fneg float %n\n", Some("%x"));
    check("  %n = fneg float %x\n  %r = fsub float -0.0, %n\n", Some("%x"));
    check("  %n = fsub float 0.0, %x\n  %r = fdiv nnan float %n, %x\n", Some("-1.0"));
    // The pin's m_FNegNSZ is `fsub +-0.0, X`, not the `fneg` instruction.
    check("  %n = fneg float %x\n  %r = fdiv nnan float %n, %x\n", None);
}

#[test]
fn reassociation_needs_reassoc_and_nsz() {
    check("  %d = fsub float %x, %y\n  %r = fadd reassoc nsz float %d, %y\n", Some("%x"));
    check("  %d = fsub float %x, %y\n  %r = fadd nsz float %d, %y\n", None);
    check("  %a = fadd float %x, %y\n  %r = fsub reassoc nsz float %a, %y\n", Some("%x"));
    check("  %m = fmul float %x, %y\n  %r = fdiv reassoc nnan float %m, %y\n", Some("%x"));
    check("  %r = fdiv nnan float %x, %x\n", Some("1.0"));
    check("  %r = fdiv nnan ninf float %x, 0.0\n", Some("poison"));
    check("  %r = frem nnan float 0.0, %x\n", Some("0.0"));
    check("  %r = frem nnan float -0.0, %x\n", Some("-0.0"));
}

/// `fcmp olt double C1, C2` stayed an instruction through the whole O2
/// pipeline (zz_f): InstSimplify had no `fcmp` case, so nothing ever asked
/// the constant folder. A NaN is unordered, an undef is chosen to be one.
#[test]
fn a_compare_of_constants_folds() {
    check("  %r = fcmp olt float 1.0, 2.0\n", Some("Int(1)"));
    check("  %r = fcmp ogt float 1.0, 2.0\n", Some("Int(0)"));
    check("  %r = fcmp olt float 1.0, 0x7FF8000000000000\n", Some("Int(0)"));
    check("  %r = fcmp ult float 1.0, 0x7FF8000000000000\n", Some("Int(1)"));
    check("  %r = fcmp ult float %x, undef\n", Some("Int(1)"));
    check("  %r = fcmp olt float %x, undef\n", Some("Int(0)"));
    check("  %r = fcmp olt float %x, poison\n", Some("poison"));
    check("  %r = fcmp true float %x, %y\n", Some("Int(1)"));
    check("  %r = fcmp olt float %x, %y\n", None);
}
