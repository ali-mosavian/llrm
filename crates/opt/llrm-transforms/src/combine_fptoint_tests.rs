//! `combine_fptoint`: `fptosi` / `fptoui` through the driver.

use llrm_mir::datalayout::DataLayout;

use crate::combine::{combine_with, visit};
use crate::testing::{parsed, printed};

fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// recfloat -Os kept `fptosi (phi double)` where the pin folds the cast into
/// the arms and ends with an integer phi (wasm +13 B, x64 +24 B vs the pin).
#[test]
fn test_fptosi_of_a_phi_with_a_constant_arm_folds_into_the_arms() {
    let after = combined(
        "define i32 @f(double %x, i1 %c) {
b0:
  br i1 %c, label %a, label %m
a:
  %y = fadd double %x, 1.0
  br label %m
m:
  %p = phi double [ 0.5, %b0 ], [ %y, %a ]
  %t = fptosi double %p to i32
  ret i32 %t
}
",
    );
    assert!(!after.contains("phi double"), "{after}");
}

/// `fptosi (sitofp X)` is X when the intermediate type holds X exactly.
#[test]
fn test_fptosi_of_an_exact_sitofp_is_the_integer() {
    let after = combined(
        "define i16 @f(i16 %x) {
b0:
  %d = sitofp i16 %x to double
  %r = fptosi double %d to i16
  ret i16 %r
}
",
    );
    assert!(after.contains("ret i16 %x"), "{after}");
}

/// A wider result is an extension, a narrower one a truncation.
#[test]
fn test_fptoui_of_a_uitofp_widens_and_narrows() {
    let wide = combined(
        "define i32 @f(i16 %x) {
b0:
  %d = uitofp i16 %x to double
  %r = fptoui double %d to i32
  ret i32 %r
}
",
    );
    assert!(wide.contains("zext i16 %x to i32"), "{wide}");
    let narrow = combined(
        "define i8 @f(i16 %x) {
b0:
  %d = uitofp i16 %x to double
  %r = fptoui double %d to i8
  ret i8 %r
}
",
    );
    assert!(narrow.contains("trunc i16 %x to i8"), "{narrow}");
}

/// i32 -> float -> i32 is not exact (24 mantissa bits), and the 32-bit result
/// is wider than the mantissa: the pair stays.
#[test]
fn test_an_inexact_round_trip_stays() {
    let after = combined(
        "define i32 @f(i32 %x) {
b0:
  %d = sitofp i32 %x to float
  %r = fptosi float %d to i32
  ret i32 %r
}
",
    );
    assert!(after.contains("fptosi") && after.contains("sitofp"), "{after}");
}

/// `fptosi (fdiv (sitofp X), C)` is `sdiv X, C` where the quotient cannot
/// round across an integer.
#[test]
fn test_fptosi_of_an_integer_fdiv_is_an_sdiv() {
    let after = combined(
        "define i16 @f(i16 %x) {
b0:
  %d = sitofp i16 %x to double
  %q = fdiv double %d, 3.0
  %r = fptosi double %q to i16
  ret i16 %r
}
",
    );
    assert!(after.contains("sdiv i16 %x, 3"), "{after}");
    assert!(!after.contains("fptosi"), "{after}");
}

/// A divisor of -1 is not taken: `sdiv INT_MIN, -1` is undefined.
#[test]
fn test_fptosi_of_an_fdiv_by_minus_one_is_not_an_sdiv() {
    let after = combined(
        "define i16 @f(i16 %x) {
b0:
  %d = sitofp i16 %x to double
  %q = fdiv double %d, -1.0
  %r = fptosi double %q to i16
  ret i16 %r
}
",
    );
    assert!(!after.contains("sdiv"), "{after}");
}
