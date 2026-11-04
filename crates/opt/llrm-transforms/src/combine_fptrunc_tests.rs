//! `combine_fptrunc`: `fptrunc` through the driver.

use llrm_mir::datalayout::DataLayout;

use crate::combine::{combine_with_declared, visit};
use crate::testing::{parsed, printed};

fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with_declared(context, &globals, &metadata, &DataLayout::default(), function, visit, &mut declared);
    declared.place(&mut module).unwrap();
    printed(&module)
}

/// `(float)((double)a + (double)b)` kept two `fpext`, a double `fadd` and an
/// `fptrunc` (m32: a `fadd` in 80 bits and a store-reload to round) where the
/// pin ends with one `fadd float`.
#[test]
fn test_fptrunc_of_a_sum_of_extended_floats_is_a_float_sum() {
    let after = combined(
        "define float @f(float %a, float %b) {
b0:
  %x = fpext float %a to double
  %y = fpext float %b to double
  %s = fadd double %x, %y
  %r = fptrunc double %s to float
  ret float %r
}
",
    );
    assert!(after.contains("fadd float %a, %b"), "{after}");
    assert!(!after.contains("fpext") && !after.contains("fptrunc"), "{after}");
}

/// A double sum of doubles is not narrowed: the sources do not fit a float.
#[test]
fn test_fptrunc_of_a_sum_of_doubles_stays() {
    let after = combined(
        "define float @f(double %a, double %b) {
b0:
  %s = fadd double %a, %b
  %r = fptrunc double %s to float
  ret float %r
}
",
    );
    assert!(after.contains("fadd double"), "{after}");
}

/// `(float)((double)a * 2.0)`: the constant fits a float, so the product is
/// taken in float.
#[test]
fn test_fptrunc_of_a_product_with_a_float_constant_is_a_float_product() {
    let after = combined(
        "define float @f(float %a) {
b0:
  %x = fpext float %a to double
  %s = fmul double %x, 2.0
  %r = fptrunc double %s to float
  ret float %r
}
",
    );
    assert!(after.contains("fmul float %a, 2.0"), "{after}");
}

/// A constant that does not fit a float keeps the double product.
#[test]
fn test_fptrunc_of_a_product_with_a_double_constant_stays() {
    let after = combined(
        "define float @f(float %a) {
b0:
  %x = fpext float %a to double
  %s = fmul double %x, 0.1
  %r = fptrunc double %s to float
  ret float %r
}
",
    );
    assert!(after.contains("fmul double"), "{after}");
}

/// `(float)fmod((double)a, (double)b)`: the remainder is exact, so it is
/// taken in float.
#[test]
fn test_fptrunc_of_a_remainder_of_extended_floats_is_a_float_remainder() {
    let after = combined(
        "define float @f(float %a, float %b) {
b0:
  %x = fpext float %a to double
  %y = fpext float %b to double
  %s = frem double %x, %y
  %r = fptrunc double %s to float
  ret float %r
}
",
    );
    assert!(after.contains("frem float %a, %b"), "{after}");
}

/// `(float)(-d)` is the pin's `fneg (fptrunc d)`.
#[test]
fn test_fptrunc_of_an_fneg_negates_the_narrow_value() {
    let after = combined(
        "define float @f(double %d) {
b0:
  %n = fneg double %d
  %r = fptrunc double %n to float
  ret float %r
}
",
    );
    let (trunc, neg) = (after.find("fptrunc"), after.find("fneg"));
    assert!(trunc.is_some() && neg.is_some() && trunc < neg, "{after}");
}

/// `(float)fabs(d)` is `fabs (fptrunc d)`.
#[test]
fn test_fptrunc_of_a_fabs_takes_the_fabs_of_the_narrow_value() {
    let after = combined(
        "define float @f(double %d) {
b0:
  %a = call double @llvm.fabs.f64(double %d)
  %r = fptrunc double %a to float
  ret float %r
}
declare double @llvm.fabs.f64(double)
",
    );
    assert!(after.contains("call float @llvm.fabs.f32(float %"), "{after}");
}

/// A rounding intrinsic narrows only when its input is an `fpext` from the
/// destination type: `(float)floor((double)f)`.
#[test]
fn test_fptrunc_of_a_floor_of_an_extended_float_is_a_float_floor() {
    let after = combined(
        "define float @f(float %a, double %d) {
b0:
  %x = fpext float %a to double
  %t = call double @llvm.floor.f64(double %x)
  %r = fptrunc double %t to float
  ret float %r
}
declare double @llvm.floor.f64(double)
",
    );
    assert!(after.contains("call float @llvm.floor.f32(float %a)"), "{after}");
    let kept = combined(
        "define float @f(double %d) {
b0:
  %t = call double @llvm.floor.f64(double %d)
  %r = fptrunc double %t to float
  ret float %r
}
declare double @llvm.floor.f64(double)
",
    );
    assert!(kept.contains("@llvm.floor.f64"), "{kept}");
}

/// `fptrunc (select c, (fpext x), y)` selects in float.
#[test]
fn test_fptrunc_of_a_select_with_an_extended_arm_selects_in_float() {
    let after = combined(
        "define float @f(i1 %c, float %a, double %d) {
b0:
  %x = fpext float %a to double
  %s = select i1 %c, double %x, double %d
  %r = fptrunc double %s to float
  ret float %r
}
",
    );
    assert!(after.contains("select i1 %c, float %a, float %"), "{after}");
}

/// `fptrunc (sitofp i16 to double) to float` converts straight to float.
#[test]
fn test_fptrunc_of_an_exact_int_conversion_converts_directly() {
    let after = combined(
        "define float @f(i16 %x) {
b0:
  %d = sitofp i16 %x to double
  %r = fptrunc double %d to float
  ret float %r
}
",
    );
    assert!(after.contains("sitofp i16 %x to float"), "{after}");
    assert!(!after.contains("fptrunc"), "{after}");
}

/// `(float)(double)(float)x` kept three casts; the pair `fptrunc (fpext y)`
/// back to `y`'s type is a same-type `bitcast` that stayed in the output.
#[test]
fn test_fptrunc_of_an_fpext_back_to_its_type_leaves_one_cast() {
    let after = combined(
        "define float @f(double %x) {
b0:
  %a = fptrunc double %x to float
  %b = fpext float %a to double
  %c = fptrunc double %b to float
  ret float %c
}
",
    );
    assert_eq!(after.matches("fptrunc").count(), 1, "{after}");
    assert!(!after.contains("fpext") && !after.contains("bitcast"), "{after}");
}

/// `n` narrowable sums, each with a float and a constant operand, chained.
fn sums(n: usize) -> String {
    let mut text = String::from("define float @f(float %a, float %b) {\nb0:\n  %s0 = fadd float %a, %b\n");
    for i in 1..=n {
        let p = i - 1;
        text += &format!(
            "  %x{i} = fpext float %s{p} to double\n  %y{i} = fpext float %b to double\n  %d{i} = fadd double %x{i}, %y{i}\n  %m{i} = fmul double %d{i}, 2.0\n  %s{i} = fptrunc double %m{i} to float\n"
        );
    }
    text + &format!("  ret float %s{n}\n}}\n")
}

/// The visitor does bounded work per cast: twice the chain costs about twice,
/// not four times (a slope test at 600 and 1200, the least of three runs).
#[test]
fn test_fptrunc_work_grows_with_the_chain_not_with_its_square() {
    let least = |n: usize| {
        let text = sums(n);
        (0..3)
            .map(|_| {
                let mut module = parsed(&text);
                let globals = module.globals.clone();
                let metadata = module.metadata.clone();
                let mut declared = llrm_mir::passes::Declared::of(&module);
                let (context, function) = module.function_mut("f").expect("@f");
                llrm_support::debug::work_of(|| {
                    combine_with_declared(
                        context,
                        &globals,
                        &metadata,
                        &DataLayout::default(),
                        function,
                        visit,
                        &mut declared,
                    )
                })
                .1 as f64
            })
            .fold(f64::MAX, f64::min)
    };
    let (small, large) = (least(600), least(1200));
    assert!(large < small * 3.0, "600 sums {small:.0} work, 1200 sums {large:.0}: a ratio of {:.1}", large / small);
}
