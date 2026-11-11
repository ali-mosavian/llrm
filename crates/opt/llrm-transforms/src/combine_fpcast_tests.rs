//! `combine_fpcast`: `uitofp`, `sitofp` and `fpext` through the driver.

use llrm_mir::datalayout::DataLayout;

use crate::combine::{combine_with, visit};
use crate::testing::{parsed, printed};

fn combined(text: &str) -> String {
    combined_in(&DataLayout::default(), text)
}

fn combined_in(
    layout: &DataLayout,
    text: &str,
) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, layout, function, visit);
    printed(&module)
}

/// `(double)(long long)int` kept `sext` + `sitofp i64` where the pin ends with
/// one `sitofp i32`: no visitor reached `commonCastTransforms`.
#[test]
fn test_sitofp_of_a_sext_converts_the_narrow_value() {
    let after = combined(
        "define double @f(i32 %x) {
b0:
  %w = sext i32 %x to i64
  %d = sitofp i64 %w to double
  ret double %d
}
",
    );
    assert!(after.contains("sitofp i32 %x to double"), "{after}");
    assert!(!after.contains("sext"), "{after}");
}

/// `(double)(int)uchar` kept `zext` + `uitofp`; the pin converts the byte.
#[test]
fn test_uitofp_of_a_zext_converts_the_narrow_value() {
    let after = combined(
        "define double @f(i8 %x) {
b0:
  %w = zext i8 %x to i32
  %d = uitofp i32 %w to double
  ret double %d
}
",
    );
    assert!(after.contains("uitofp i8 %x to double"), "{after}");
    assert!(!after.contains("zext"), "{after}");
}

/// A `sitofp` of a value known non-negative is a `uitofp nneg`.
#[test]
fn test_sitofp_of_a_non_negative_is_uitofp_nneg() {
    let after = combined(
        "define double @f(i32 %x) {
b0:
  %m = and i32 %x, 255
  %d = sitofp i32 %m to double
  ret double %d
}
",
    );
    assert!(after.contains("uitofp nneg i32 %m to double"), "{after}");
}

/// A `uitofp` of a value known non-negative gains `nneg`.
#[test]
fn test_uitofp_of_a_non_negative_gains_nneg() {
    let after = combined(
        "define double @f(i32 %x) {
b0:
  %m = and i32 %x, 255
  %d = uitofp i32 %m to double
  ret double %d
}
",
    );
    assert!(after.contains("uitofp nneg i32 %m to double"), "{after}");
}

/// `uitofp (and (trunc X), Mask)` converts the masked wide value when the
/// narrow type is illegal.
#[test]
fn test_uitofp_of_a_masked_illegal_trunc_converts_the_wide_value() {
    let after = combined_in(
        &DataLayout::parse("e-n8:16:32:64").expect("a layout"),
        "define double @f(i64 %x) {
b0:
  %t = trunc i64 %x to i24
  %m = and i24 %t, 255
  %d = uitofp i24 %m to double
  ret double %d
}
",
    );
    assert!(!after.contains("trunc"), "{after}");
    assert!(after.contains("and i64 %x, 255"), "{after}");
}

/// `fpext (sitofp i16 to float)` is exact: the pin converts straight to
/// double. An i32 does not fit a float and stays.
#[test]
fn test_fpext_of_an_exact_int_conversion_converts_directly() {
    let exact = combined(
        "define double @f(i16 %x) {
b0:
  %d = sitofp i16 %x to float
  %e = fpext float %d to double
  ret double %e
}
",
    );
    assert!(exact.contains("sitofp i16 %x to double"), "{exact}");
    assert!(!exact.contains("fpext"), "{exact}");
    let inexact = combined(
        "define double @f(i32 %x) {
b0:
  %d = sitofp i32 %x to float
  %e = fpext float %d to double
  ret double %e
}
",
    );
    assert!(inexact.contains("fpext"), "{inexact}");
}
