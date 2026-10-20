use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with};
use crate::combine_compares::visit_icmp;
use crate::testing::{parsed, printed};

/// `visitICmpInst` alone, through the driver.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if matches!(c.function.instruction(inst).opcode, Opcode::ICmp(_)) { visit_icmp(c, inst) } else { Visit::None }
}

/// `@f(params)` returning i1 after `visitICmpInst` alone must have each of
/// `wanted`: the shape pinned `opt -passes=instcombine` gives.
fn check(
    params: &str,
    body: &str,
    wanted: &[&str],
) {
    let mut module = parsed(&format!("define i1 @f({params}) {{\nb0:\n{body}  ret i1 %r\n}}\n"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    let out = printed(&module);
    for one in wanted {
        assert!(out.contains(one), "`{one}` missing\n{out}");
    }
    assert!(!out.contains(" and ") || wanted.iter().any(|w| w.contains("and")), "an `and` is left\n{out}");
}

/// stft: `(a | b) & signbit != 0` stayed a mask and a compare, where the pin
/// has `(a | b) < 0`.
#[test]
fn a_sign_mask_test_is_a_compare_with_zero() {
    check("i16 %x", "  %m = and i16 %x, -32768\n  %r = icmp ne i16 %m, 0\n", &["icmp slt i16 %x, 0"]);
    check("i16 %x", "  %m = and i16 %x, -32768\n  %r = icmp eq i16 %m, 0\n", &["icmp sgt i16 %x, -1"]);
}

#[test]
fn a_high_mask_equal_to_zero_is_an_unsigned_bound() {
    check("i16 %x", "  %m = and i16 %x, -16\n  %r = icmp eq i16 %m, 0\n", &["icmp ult i16 %x, 16"]);
}

#[test]
fn the_low_bit_not_zero_is_a_truncate() {
    check("i16 %x", "  %m = and i16 %x, 1\n  %r = icmp ne i16 %m, 0\n", &["trunc i16 %x to i1"]);
}

#[test]
fn x_minus_one_and_not_x_negative_is_x_zero() {
    check(
        "i16 %x",
        "  %a = add i16 %x, -1\n  %n = xor i16 %x, -1\n  %m = and i16 %a, %n\n  %r = icmp slt i16 %m, 0\n",
        &["icmp eq i16 %x, 0"],
    );
}

#[test]
fn x_and_negated_x_equal_to_zero_is_x_zero() {
    check("i16 %x", "  %n = sub i16 0, %x\n  %m = and i16 %x, %n\n  %r = icmp eq i16 %m, 0\n", &["icmp eq i16 %x, 0"]);
}

#[test]
fn a_masked_shift_compare_moves_the_shift_into_the_constants() {
    check(
        "i16 %x",
        "  %s = lshr i16 %x, 3\n  %m = and i16 %s, 5\n  %r = icmp ne i16 %m, 1\n",
        &["and i16 %x, 40", ", 8"],
    );
}

#[test]
fn a_masked_shift_left_compare_moves_the_shift_into_the_constants() {
    check(
        "i16 %x",
        "  %s = shl i16 %x, 3\n  %m = and i16 %s, 40\n  %r = icmp eq i16 %m, 0\n",
        &["and i16 %x, 5", "icmp eq i16 %"],
    );
}

#[test]
fn an_and_with_a_shifted_all_ones_is_a_shift_of_the_other_operand() {
    check(
        "i16 %x, i16 %y",
        "  %s = shl i16 -1, %x\n  %m = and i16 %s, %y\n  %r = icmp eq i16 %m, 0\n",
        &["lshr i16 %y, %x", "icmp eq i16 %"],
    );
}

#[test]
fn a_negated_power_of_two_mask_equal_to_itself_is_an_unsigned_bound() {
    check("i16 %x", "  %m = and i16 %x, -8\n  %r = icmp eq i16 %m, -8\n", &["icmp ugt i16 %x, -9"]);
}

/// stft_single/stft_fixed: two sign tests ORed stayed a masked test of the
/// OR; through the whole driver it is `(a | b) < 0`.
#[test]
fn two_sign_tests_ored_are_one_sign_test_of_the_or() {
    let mut module = parsed(
        "define i1 @f(i16 %a, i16 %b) {\nb0:\n  %x = icmp slt i16 %a, 0\n  %y = icmp slt i16 %b, 0\n  %r = or i1 %x, %y\n  ret i1 %r\n}\n",
    );
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    crate::combine::combine(context, &globals, &metadata, &DataLayout::default(), function, &mut declared);
    declared.place(&mut module).expect("placed");
    let out = printed(&module);
    assert!(out.contains("or i16 %a, %b") && out.contains("icmp slt i16 %"), "{out}");
    assert!(!out.contains(" and "), "{out}");
}
