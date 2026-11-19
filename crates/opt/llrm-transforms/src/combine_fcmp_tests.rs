//! `combine_fcmp`: `fcmp` through the driver. The first three tests are the
//! compare-with-infinity canonicalisation, which no visitor made before.

use llrm_mir::datalayout::DataLayout;

use crate::combine::{combine_with, visit};
use crate::testing::{parsed, printed};

fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let layout = DataLayout::default();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &layout, function, visit);
    printed(&module)
}

fn one(
    pred_and_rhs: &str,
    expect: &str,
) {
    let after = combined(&format!(
        "define i1 @f(float %x) {{\nb0:\n  %c = fcmp {pred_and_rhs}\n  ret i1 %c\n}}\n"
    ));
    assert!(after.contains(expect), "{pred_and_rhs}: {after}");
}

/// `x < +inf` stayed `olt`; the pin makes it `one` (false only for NaN and +inf).
#[test]
fn test_olt_plus_inf_is_one() {
    one("olt float %x, 0x7FF0000000000000", "fcmp one float %x, 0x7FF0000000000000");
}

/// `x >= -inf` stayed `oge`; the pin's `fcmp ord x, 0.0` (NaN check only).
#[test]
fn test_oge_minus_inf_is_ord_zero() {
    one("oge float %x, 0xFFF0000000000000", "fcmp ord float %x, 0.000000e+00");
}

/// `x >= +inf` is `x == +inf`.
#[test]
fn test_oge_plus_inf_is_oeq() {
    one("oge float %x, 0x7FF0000000000000", "fcmp oeq float %x, 0x7FF0000000000000");
}

/// `fcmp ole x, -0.0` kept the negative zero: fcmp ignores its sign, the pin
/// writes +0.0.
#[test]
fn test_a_negative_zero_operand_becomes_positive_zero() {
    one("ole float %x, -0.000000e+00", "fcmp ole float %x, 0.000000e+00");
}

/// `fcmp ult x, x` is true when x is NaN: `fcmp uno x, 0.0`.
#[test]
fn test_ult_of_a_value_with_itself_is_uno_zero() {
    one("ult float %x, %x", "fcmp uno float %x, 0.000000e+00");
}

/// `fcmp olt (fneg x), (fneg y)` is `fcmp ogt x, y`.
#[test]
fn test_two_negations_swap_the_predicate() {
    let after = combined(
        "define i1 @f(float %x, float %y) {
b0:
  %a = fneg float %x
  %b = fneg float %y
  %c = fcmp olt float %a, %b
  ret i1 %c
}
",
    );
    assert!(after.contains("fcmp ogt float %x, %y"), "{after}");
}

/// A compare used only by a min/max select keeps its form: the -0.0 is not
/// rewritten, so the select stays the idiom the selector knows.
#[test]
fn test_a_min_max_compare_is_left_alone() {
    let after = combined(
        "define float @f(float %x) {
b0:
  %c = fcmp ogt float %x, -0.000000e+00
  %s = select i1 %c, float %x, float -0.000000e+00
  ret float %s
}
",
    );
    assert!(after.contains("-0.000000e+00"), "{after}");
}

/// `fcmp uno` of two integer conversions (never NaN) left a constant
/// `fcmp uno 0.0, 0.0` to the backend (wasm sfm16_float -Os: `f32.ne` of two
/// zeros). The pin folds it to false.
#[test]
fn test_uno_of_two_never_nan_values_folds_to_false() {
    let after = combined(
        "define i1 @f(i32 %a, i32 %b) {\nb0:\n  %x = sitofp i32 %a to float\n  %y = sitofp i32 %b to float\n  \
         %c = fcmp uno float %x, %y\n  ret i1 %c\n}\n",
    );
    assert!(!after.contains("fcmp"), "{after}");
}

/// The same, through `une x, x` of a conversion.
#[test]
fn test_une_of_a_never_nan_value_with_itself_folds_to_false() {
    let after = combined(
        "define i1 @f(i32 %a) {\nb0:\n  %x = sitofp i32 %a to float\n  %c = fcmp une float %x, %x\n  ret i1 %c\n}\n",
    );
    assert!(!after.contains("fcmp"), "{after}");
}

/// `fcmp oge <4 x float> %x, %x` panicked ("an fcmp compares floats") and took
/// the whole compile with it; the pin makes it `fcmp ord %x, zeroinitializer`.
#[test]
fn a_vector_compare_of_a_value_with_itself_is_ord_of_zero() {
    let after = combined(
        "define <4 x i1> @f(<4 x float> %x) {\nb0:\n  %c = fcmp oge <4 x float> %x, %x\n  ret <4 x i1> %c\n}\n",
    );
    assert!(after.contains("fcmp ord <4 x float> %x, zeroinitializer"), "{after}");
}
