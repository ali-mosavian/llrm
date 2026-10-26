use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with};
use crate::combine_compares::visit_icmp;
use crate::testing::{parsed, printed};

fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if matches!(c.function.instruction(inst).opcode, Opcode::ICmp(_)) { visit_icmp(c, inst) } else { Visit::None }
}

/// `@f` after `visitICmpInst` alone.
fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// `body` of `define i1 @f(i16 %a, i16 %b)` after `visitICmpInst`, as `opt`
/// (instcombine, one iteration) prints it.
fn check(
    body: &str,
    after: &str,
) {
    let text = format!("define i1 @f(i16 %a, i16 %b) {{\nb0:\n{body}}}\n");
    let want = format!("define i1 @f(i16 %a, i16 %b) {{\nb0:\n{after}}}\n");
    assert_eq!(combined(&text), want);
}

/// `(X urem Y) == X` is `X u< Y` (`opt`: `icmp ult i16 %a, %b`).
#[test]
fn a_urem_equal_to_its_dividend_is_a_compare_of_the_operands() {
    check(
        "  %r = urem i16 %a, %b\n  %g = icmp eq i16 %r, %a\n  ret i1 %g\n",
        "  %0 = icmp ult i16 %a, %b\n  ret i1 %0\n",
    );
}

/// `(Op1 + X) u< Op1` is `~Op1 u> X`'s form: a `not` and a compare, the
/// constant case `~C - X`.
#[test]
fn an_unsigned_overflow_check_of_a_sum_is_a_compare_with_not() {
    check(
        "  %s = add i16 %a, %b\n  %g = icmp ult i16 %s, %a\n  ret i1 %g\n",
        "  %0 = xor i16 %a, -1\n  %1 = icmp ugt i16 %b, %0\n  ret i1 %1\n",
    );
    check(
        "  %s = add i16 %b, %a\n  %g = icmp ule i16 %a, %s\n  ret i1 %g\n",
        "  %0 = xor i16 %a, -1\n  %1 = icmp ule i16 %b, %0\n  ret i1 %1\n",
    );
    check(
        "  %t = add i16 %a, %b\n  %s = add i16 %t, 7\n  %g = icmp uge i16 %s, %a\n  ret i1 %g\n",
        "  %0 = sub i16 -8, %b\n  %1 = icmp uge i16 %0, %a\n  ret i1 %1\n",
    );
}

/// `icmp (A+B), A` is `icmp B, 0`, and two sums that share a term compare the
/// others, where the sum cannot wrap in the compare's signedness.
#[test]
fn a_sum_without_wrap_against_one_of_its_terms_drops_it() {
    check(
        "  %s = add nsw i16 %a, %b\n  %g = icmp slt i16 %s, %a\n  ret i1 %g\n",
        "  %0 = icmp slt i16 %b, 0\n  ret i1 %0\n",
    );
    check(
        "  %s = add nsw i16 %a, %b\n  %t = add nsw i16 %a, 9\n  %g = icmp slt i16 %s, %t\n  ret i1 %g\n",
        "  %0 = icmp slt i16 %b, 9\n  ret i1 %0\n",
    );
}

/// quicksort.bas -Os: `lo > hi + -1` (nsw) stayed as it was where the pin's
/// InstCombine makes it `lo >= hi`; SimplifyCFG then drops the guard `lo < hi`
/// implies (one compare and branch in the entry, +17 B over main).
#[test]
fn a_compare_against_a_step_of_one_swaps_strict_for_non_strict() {
    check(
        "  %d = add nsw i16 %b, -1\n  %g = icmp sgt i16 %a, %d\n  ret i1 %g\n",
        "  %0 = icmp sge i16 %a, %b\n  ret i1 %0\n",
    );
    check(
        "  %d = add nsw i16 %a, -1\n  %g = icmp sge i16 %d, %b\n  ret i1 %g\n",
        "  %0 = icmp sgt i16 %a, %b\n  ret i1 %0\n",
    );
    check(
        "  %d = add nsw i16 %b, 1\n  %g = icmp slt i16 %a, %d\n  ret i1 %g\n",
        "  %0 = icmp sle i16 %a, %b\n  ret i1 %0\n",
    );
    check(
        "  %d = add nuw i16 %b, 1\n  %g = icmp ult i16 %a, %d\n  ret i1 %g\n",
        "  %0 = icmp ule i16 %a, %b\n  ret i1 %0\n",
    );
}

/// `icmp (A + C1), (C + C2)` is `icmp (A + C1-C2), C` where C1 is the greater
/// in magnitude, else `icmp A, (C + C2-C1)`; an equality takes the difference
/// whatever the signs.
#[test]
fn two_sums_with_constants_compare_one_sum_and_the_other_term() {
    check(
        "  %s = add nsw i16 %a, 5\n  %t = add nsw i16 %b, 3\n  %g = icmp slt i16 %s, %t\n  ret i1 %g\n",
        "  %0 = add nsw i16 %a, 2\n  %1 = icmp slt i16 %0, %b\n  ret i1 %1\n",
    );
    check(
        "  %s = add nsw i16 %a, 3\n  %t = add nsw i16 %b, 5\n  %g = icmp slt i16 %s, %t\n  ret i1 %g\n",
        "  %0 = add nsw i16 %b, 2\n  %1 = icmp slt i16 %a, %0\n  ret i1 %1\n",
    );
    check(
        "  %s = add i16 %a, 5\n  %t = add i16 %b, 3\n  %g = icmp eq i16 %s, %t\n  ret i1 %g\n",
        "  %0 = add i16 %a, 2\n  %1 = icmp eq i16 %0, %b\n  ret i1 %1\n",
    );
}

/// `icmp (A-B), A` is `icmp 0, B` with no wrap; the unsigned forms need B
/// non-zero for the non-strict flip.
#[test]
fn a_difference_against_its_minuend_compares_the_subtrahend() {
    check(
        "  %s = sub nsw i16 %a, %b\n  %g = icmp slt i16 %s, %a\n  ret i1 %g\n",
        "  %0 = icmp sgt i16 %b, 0\n  ret i1 %0\n",
    );
    check(
        "  %s = sub i16 %a, %b\n  %g = icmp ugt i16 %s, %a\n  ret i1 %g\n",
        "  %0 = icmp ugt i16 %b, %a\n  ret i1 %0\n",
    );
    check(
        "  %s = sub i16 %a, %b\n  %g = icmp ult i16 %a, %s\n  ret i1 %g\n",
        "  %0 = icmp ult i16 %a, %b\n  ret i1 %0\n",
    );
    check(
        "  %n = or i16 %b, 1\n  %s = sub i16 %a, %n\n  %g = icmp uge i16 %s, %a\n  ret i1 %g\n",
        "  %n = or i16 %b, 1\n  %0 = icmp ugt i16 %n, %a\n  ret i1 %0\n",
    );
}

/// `(0-X) < C` is `X > -C`; `(X & -4) == INT_MIN` is `X s< INT_MIN+4`; `((b +
/// 7) & 7) u< b` is `b != 0`.
#[test]
fn a_negation_a_sign_mask_and_an_offset_mask_compare_the_operand() {
    check(
        "  %s = sub nsw i16 0, %a\n  %g = icmp slt i16 %s, 5\n  ret i1 %g\n",
        "  %0 = icmp sgt i16 %a, -5\n  ret i1 %0\n",
    );
    check(
        "  %s = and i16 %a, -4\n  %g = icmp eq i16 %s, -32768\n  ret i1 %g\n",
        "  %0 = icmp slt i16 %a, -32764\n  ret i1 %0\n",
    );
    check(
        "  %t = add i16 %b, 7\n  %s = and i16 %t, 7\n  %g = icmp ult i16 %s, %b\n  ret i1 %g\n",
        "  %0 = icmp ne i16 %b, 0\n  ret i1 %0\n",
    );
}
