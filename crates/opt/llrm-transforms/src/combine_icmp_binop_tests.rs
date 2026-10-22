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

/// `@f(params)` returning i1 after `visitICmpInst` alone, printed.
fn after(
    params: &str,
    body: &str,
) -> String {
    let mut module = parsed(&format!("define i1 @f({params}) {{\nb0:\n{body}  ret i1 %r\n}}\n"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// The shape pinned `opt -passes=instcombine` gives (tools/pairs/pin.py, m32).
fn check(
    params: &str,
    body: &str,
    wanted: &str,
) {
    let out = after(params, body);
    assert!(out.contains(wanted), "`{wanted}` missing\n{out}");
}

/// x_heapsort's `_sift`: the loop test `2 * i + 1 <= n` stayed an `or
/// disjoint` and a compare, where the pin tests `2 * i < n` before the loop is
/// rotated, so the inliner, rotate and the unroll see the pin's loop.
#[test]
fn a_disjoint_or_of_one_under_a_non_strict_compare_is_the_strict_compare() {
    check("i32 %x, i32 %n", "  %o = or disjoint i32 %x, 1\n  %r = icmp sle i32 %o, %n\n", "icmp slt i32 %x, %n");
    check("i32 %x, i32 %n", "  %o = or disjoint i32 %x, 1\n  %r = icmp sgt i32 %o, %n\n", "icmp sge i32 %x, %n");
}

#[test]
fn a_sum_of_one_that_cannot_wrap_turns_the_strictness_of_the_compare() {
    check("i32 %x, i32 %n", "  %o = add nsw i32 %x, 1\n  %r = icmp sle i32 %o, %n\n", "icmp slt i32 %x, %n");
    check("i32 %x, i32 %n", "  %o = add nsw i32 %x, 1\n  %r = icmp sgt i32 %o, %n\n", "icmp sge i32 %x, %n");
    check("i32 %x, i32 %n", "  %o = add nuw i32 %x, 1\n  %r = icmp ule i32 %o, %n\n", "icmp ult i32 %x, %n");
}

/// The pin keeps `x + 1 s< n`: the flip is only for the sums it turns.
#[test]
fn a_sum_that_may_wrap_or_is_the_wrong_side_of_the_strictness_stays() {
    check("i32 %x, i32 %n", "  %o = add i32 %x, 1\n  %r = icmp sle i32 %o, %n\n", "icmp sle i32 %o, %n");
    check("i32 %x, i32 %n", "  %o = add nsw i32 %x, 1\n  %r = icmp slt i32 %o, %n\n", "icmp slt i32 %o, %n");
    check("i32 %x, i32 %n", "  %o = add nsw i32 %x, 3\n  %r = icmp sle i32 %o, %n\n", "icmp sle i32 %o, %n");
}

#[test]
fn a_sum_compared_with_its_own_term_is_a_compare_of_the_other_term() {
    check("i32 %x, i32 %y", "  %o = add nsw i32 %x, %y\n  %r = icmp slt i32 %o, %x\n", "icmp slt i32 %y, 0");
    check("i32 %x, i32 %y", "  %o = add i32 %x, %y\n  %r = icmp eq i32 %o, %x\n", "icmp eq i32 %y, 0");
}
