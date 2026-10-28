use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitFDiv`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    match c.function.instruction(inst).opcode {
        Opcode::Binary(BinaryOp::FDiv) => super::visit_fdiv(c, inst),
        Opcode::Binary(BinaryOp::FAdd | BinaryOp::FMul) => super::visit_fadd_or_fmul(c, inst),
        Opcode::Binary(BinaryOp::FSub) => super::visit_fsub(c, inst),
        // `simplifyCastInst`, as every visit starts with.
        Opcode::Cast(_) => crate::combine_muldivrem::simplify_first(c, inst).map_or(Visit::None, Visit::Replace),
        _ => Visit::None,
    }
}

/// `@f(ty %x)` returning `ty` after `visitFDiv` alone.
fn combined(
    ty: &str,
    body: &str,
) -> String {
    let mut module = parsed(&format!("define {ty} @f({ty} %x) {{\nb0:\n{body}  ret {ty} %r\n}}\n"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// Pinned `opt -passes=instcombine`: `fdiv arcp double %x, 3.0` is
/// `fmul arcp double %x, 0x3FD5555555555555`.
#[test]
fn test_a_division_by_a_normal_constant_with_arcp_is_a_multiply_by_its_reciprocal() {
    let out = combined("double", "  %r = fdiv arcp double %x, 3.0\n");
    assert!(out.contains("fmul arcp double %x, 0x3FD5555555555555"), "{out}");
    assert!(!out.contains("fdiv"), "{out}");
}

/// A power of two has an exact inverse: no flag is needed, and the product
/// keeps the flags the division had (`fdiv double %x, 4.0` is `fmul double
/// %x, 2.500000e-01`; `fdiv nnan float %x, 8.0` is `fmul nnan float %x,
/// 1.25e-1`).
#[test]
fn test_a_division_by_a_power_of_two_is_a_multiply_without_a_flag() {
    let out = combined("double", "  %r = fdiv double %x, 4.0\n");
    assert!(out.contains("fmul double %x, 2.500000e-01"), "{out}");
    let single = combined("float", "  %r = fdiv nnan float %x, 8.0\n");
    assert!(single.contains("fmul nnan float %x, 1.250000e-01"), "{single}");
}

/// Without `arcp` and without an exact inverse the division stays, and so does
/// a division by zero under `arcp` (not a normal number).
#[test]
fn test_a_division_keeps_its_operation_where_the_reciprocal_is_inexact_or_not_normal() {
    let plain = combined("double", "  %r = fdiv double %x, 3.0\n");
    assert!(plain.contains("fdiv double %x, 3.000000e+00"), "{plain}");
    let zero = combined("double", "  %r = fdiv arcp double %x, 0.0\n");
    assert!(zero.contains("fdiv arcp double %x, 0.000000e+00"), "{zero}");
}

/// x86_fp80 constants are held as doubles: an exact inverse is formed
/// (pinned: `fdiv x86_fp80 %x, 8.0` is `fmul x86_fp80 %x, 1.250000e-01`), an
/// inexact one with `arcp` is not (DEVIATE 2).
#[test]
fn test_an_x87_division_is_a_multiply_only_by_an_exact_inverse() {
    let exact = combined("x86_fp80", "  %r = fdiv x86_fp80 %x, 8.0\n");
    assert!(exact.contains("fmul x86_fp80 %x, 1.250000e-01"), "{exact}");
    let inexact = combined("x86_fp80", "  %r = fdiv arcp x86_fp80 %x, 3.0\n");
    assert!(inexact.contains("fdiv arcp x86_fp80"), "{inexact}");
}

/// A long double literal is `fpext double -8.0 to x86_fp80`: widening is exact,
/// so the operand is the constant `-8.0` and the division a multiply by
/// `-1.25e-1` (tests/check/c/divide_by_power_of_two32.c; the pin folds the
/// `fpext` in `ConstantFoldCastInstruction`).
#[test]
fn test_a_widened_constant_is_a_constant_so_its_division_is_a_multiply() {
    let out = combined("x86_fp80", "  %w = fpext double -8.0 to x86_fp80\n  %r = fdiv x86_fp80 %x, %w\n");
    assert!(out.contains("fmul x86_fp80 %x, -1.250000e-01"), "{out}");
    assert!(!out.contains("fpext") && !out.contains("fdiv"), "{out}");
}

/// Pinned `opt -passes=instcombine`: `(x + 1.0) + 2.0` with `reassoc nsz` on
/// the outer operation is `x + 3.0` (the flags of the outer one stay), a
/// product likewise; `reassoc` alone, or `nsz` alone, is no license.
#[test]
fn test_a_constant_chain_folds_only_with_reassoc_and_nsz() {
    let sum = combined("double", "  %a = fadd double %x, 1.0\n  %r = fadd reassoc nsz double %a, 2.0\n");
    assert!(sum.contains("fadd reassoc nsz double %x, 3.000000e+00") && sum.matches("fadd").count() == 1, "{sum}");
    let product =
        combined("double", "  %a = fmul reassoc nsz double %x, 2.0\n  %r = fmul reassoc nsz double %a, 4.0\n");
    assert!(
        product.contains("fmul reassoc nsz double %x, 8.000000e+00") && product.matches("fmul").count() == 1,
        "{product}"
    );
    let reassoc_only = combined("double", "  %a = fadd reassoc double %x, 1.0\n  %r = fadd reassoc double %a, 2.0\n");
    assert_eq!(reassoc_only.matches("fadd").count(), 2, "{reassoc_only}");
    let inner_only = combined("double", "  %a = fadd reassoc nsz double %x, 1.0\n  %r = fadd double %a, 2.0\n");
    assert_eq!(inner_only.matches("fadd").count(), 2, "{inner_only}");
}

/// A constant goes to the right of a sum or a product.
#[test]
fn test_a_float_constant_goes_to_the_right_of_a_commutative_operation() {
    let out = combined("double", "  %r = fmul double 2.0, %x\n");
    assert!(out.contains("fmul double %x, 2.000000e+00"), "{out}");
}

/// Pinned `opt -passes=instcombine`: `fsub float %x, 1.5` is `fadd float %x,
/// -1.5`. Left as a subtract, 26 QCport functions differed from the pin by
/// this alone.
#[test]
fn test_a_subtraction_of_a_constant_is_the_addition_of_its_negation() {
    let out = combined("float", "  %r = fsub float %x, 1.5\n");
    assert!(out.contains("fadd float %x, -1.5"), "{out}");
    let out = combined("double", "  %r = fsub fast double %x, -2.0\n");
    assert!(out.contains("fadd fast double %x, 2.0"), "{out}");
}

/// Pinned: `fsub -0.0, %x` is `fneg %x` and `%x - (fneg %y)` is `%x + %y`.
#[test]
fn test_a_subtraction_from_negative_zero_is_a_negation_and_of_a_negation_an_addition() {
    let out = combined("double", "  %r = fsub double -0.0, %x\n");
    assert!(out.contains("fneg double %x"), "{out}");
    let out = combined("double", "  %n = fneg double %x\n  %r = fsub double %x, %n\n");
    assert!(out.contains("fadd double %x, %x"), "{out}");
}
