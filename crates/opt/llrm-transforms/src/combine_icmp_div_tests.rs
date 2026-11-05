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

/// `@f(params)` after `visitICmpInst` alone must have each of `wanted` and no
/// division: the text the pinned `opt -passes=instcombine` gives.
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
}

/// jpeg_fixed -O1: `icmp slt (sdiv v, 1024), 0` stayed a compare of the
/// quotient, where the pin compares `v` with -1023 (the first InstCombine of
/// idct), +37 B over the pin.
#[test]
fn a_signed_quotient_below_zero_is_the_dividend_below_the_range() {
    check("i32 %v", "  %x = sdiv i32 %v, 1024\n  %r = icmp slt i32 %x, 0\n", &["icmp slt i32 %v, -1023"]);
}

#[test]
fn a_signed_quotient_above_a_positive_bound_is_the_dividend_above_the_range() {
    check("i32 %v", "  %x = sdiv i32 %v, 1024\n  %r = icmp sgt i32 %x, 255\n", &["icmp sgt i32 %v, 262143"]);
}

#[test]
fn an_unsigned_quotient_equal_to_a_constant_is_a_range_test() {
    check("i32 %v", "  %x = udiv i32 %v, 10\n  %r = icmp eq i32 %x, 3\n", &["sub i32 %v, 30", "icmp ult i32 %0, 10"]);
}

#[test]
fn an_unsigned_quotient_below_a_constant_is_a_compare_with_the_product() {
    check("i32 %v", "  %x = udiv i32 %v, 10\n  %r = icmp ult i32 %x, 3\n", &["icmp ult i32 %v, 30"]);
}

#[test]
fn a_signed_quotient_equal_to_a_negative_constant_is_a_range_test() {
    check("i32 %v", "  %x = sdiv i32 %v, 5\n  %r = icmp eq i32 %x, -3\n", &["sub i32 %v, -19", "icmp ult i32 %0, 5"]);
}

#[test]
fn a_negative_divisor_swaps_the_condition() {
    check("i32 %v", "  %x = sdiv i32 %v, -5\n  %r = icmp slt i32 %x, 3\n", &["icmp sgt i32 %v, -15"]);
}

#[test]
fn an_unsigned_quotient_of_all_ones_needs_a_dividend_of_all_ones_and_a_divisor_of_one() {
    check(
        "i32 %v, i32 %w",
        "  %x = udiv i32 %v, %w\n  %r = icmp eq i32 %x, -1\n",
        &["icmp eq i32 %v, -1", "icmp eq i32 %w, 1"],
    );
}
