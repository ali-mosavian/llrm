use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use super::visit_fneg;
use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visitFNeg` alone, through the driver.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::FNeg { visit_fneg(c, inst) } else { Visit::None }
}

/// `define RET @f(PARAMS)` with `body` before and `after` after `visitFNeg`
/// alone (`opt -passes=instcombine` gives `after` for the same `body`).
fn check(
    params: &str,
    body: &str,
    after: &str,
) {
    let text = format!("define float @f({params}) {{\nb0:\n{body}}}\n");
    let want = format!("define float @f({params}) {{\nb0:\n{after}}}\n");
    let mut module = parsed(&text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    assert_eq!(printed(&module), want);
}

/// `stft_single -O1`: `fneg (fmul x, C)` stayed an `fneg` of the product, one
/// `f32.neg` more per butterfly; `opt` folds the sign into the constant.
#[test]
fn a_negated_product_with_a_constant_negates_the_constant() {
    check(
        "float %x",
        "  %m = fmul float %x, 2.0\n  %n = fneg float %m\n  ret float %n\n",
        "  %0 = fmul float %x, -2.000000e+00\n  ret float %0\n",
    );
}

/// The flags of the product: the fneg's value flags union the product's, `ninf`
/// only if both have it, rewrite flags only if both (`opt`: `fmul nsz`).
#[test]
fn the_folded_product_keeps_the_union_of_value_flags_and_ninf_of_both() {
    check(
        "float %x",
        "  %m = fmul reassoc nsz float %x, 2.0\n  %n = fneg ninf float %m\n  ret float %n\n",
        "  %0 = fmul nsz float %x, -2.000000e+00\n  ret float %0\n",
    );
}

#[test]
fn a_negated_quotient_by_a_constant_divides_by_the_negated_constant() {
    check(
        "float %x",
        "  %m = fdiv float %x, 4.0\n  %n = fneg float %m\n  ret float %n\n",
        "  %0 = fdiv float %x, -4.000000e+00\n  ret float %0\n",
    );
}

#[test]
fn a_negated_quotient_of_a_constant_negates_the_dividend() {
    check(
        "float %x",
        "  %m = fdiv float 4.0, %x\n  %n = fneg float %m\n  ret float %n\n",
        "  %0 = fdiv float -4.000000e+00, %x\n  ret float %0\n",
    );
}

#[test]
fn under_nsz_a_negated_sum_with_a_constant_is_the_negated_constant_minus_the_value() {
    check(
        "float %x",
        "  %m = fadd float %x, 2.0\n  %n = fneg nsz float %m\n  ret float %n\n",
        "  %0 = fsub nsz float -2.000000e+00, %x\n  ret float %0\n",
    );
}

#[test]
fn under_nsz_a_negated_difference_is_the_swapped_difference() {
    check(
        "float %x, float %y",
        "  %m = fsub float %x, %y\n  %n = fneg nsz float %m\n  ret float %n\n",
        "  %0 = fsub nsz float %y, %x\n  ret float %0\n",
    );
}

#[test]
fn a_negated_product_of_two_values_negates_the_right_factor() {
    check(
        "float %x, float %y",
        "  %m = fmul float %x, %y\n  %n = fneg float %m\n  ret float %n\n",
        "  %0 = fneg float %y\n  %1 = fmul float %x, %0\n  ret float %1\n",
    );
}

#[test]
fn a_negated_quotient_of_two_values_negates_the_dividend() {
    check(
        "float %x, float %y",
        "  %m = fdiv float %x, %y\n  %n = fneg float %m\n  ret float %n\n",
        "  %0 = fneg float %x\n  %1 = fdiv float %0, %y\n  ret float %1\n",
    );
}

#[test]
fn a_negated_select_with_a_negated_true_arm_negates_the_other_arm() {
    check(
        "i1 %c, float %x, float %y",
        "  %p = fneg float %x\n  %s = select i1 %c, float %p, float %y\n  %n = fneg float %s\n  ret float %n\n",
        "  %0 = fneg float %y\n  %1 = select i1 %c, float %x, float %0\n  ret float %1\n",
    );
}

#[test]
fn a_negated_select_with_a_constant_arm_negates_both_arms() {
    check(
        "i1 %c, float %x",
        "  %s = select i1 %c, float %x, float 3.0\n  %n = fneg float %s\n  ret float %n\n",
        "  %0 = fneg float %x\n  %1 = select i1 %c, float %0, float -3.000000e+00\n  ret float %1\n",
    );
}

/// An `fneg` of a value with another reader stays (`m_OneUse`).
#[test]
fn a_negated_product_read_elsewhere_is_kept() {
    let text = "define float @f(float %x) {\nb0:\n  %m = fmul float %x, 2.000000e+00\n  %n = fneg float %m\n  %r = fadd float %n, %m\n  ret float %r\n}\n";
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    assert_eq!(printed(&module), text);
}
