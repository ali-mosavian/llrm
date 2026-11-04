use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitAnd` and `visitOr`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    match c.function.instruction(inst).opcode {
        Opcode::Binary(BinaryOp::And) => crate::combine_and::visit_and(c, inst),
        Opcode::Binary(BinaryOp::Or) => crate::combine_or::visit_or(c, inst),
        _ => Visit::None,
    }
}

/// `@f(params)` returning i1 after the `and`/`or` visits alone.
fn combined(
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

/// `body` must come out with each of `wanted`, the shape pinned
/// `opt -passes=instcombine` gives.
fn check(
    params: &str,
    body: &str,
    wanted: &[&str],
) {
    let out = combined(params, body);
    for one in wanted {
        assert!(out.contains(one), "`{one}` missing\n{out}");
    }
}

const AB: &str = "i16 %a, i16 %b";

/// stft: `(a < 0) | (b < 0)` stayed two compares and a combining `or`; the
/// sign bit is the common mask of the two bit tests. The pin's `icmp` visit
/// then turns `(x & signbit) != 0` into `x < 0` (`foldICmpAndConstant`, not yet
/// translated), so the test stops at the masked form.
#[test]
fn two_sign_tests_ored_are_one_test_of_the_or() {
    check(
        AB,
        "  %x = icmp slt i16 %a, 0\n  %y = icmp slt i16 %b, 0\n  %r = or i1 %x, %y\n",
        &["or i16 %a, %b", "and i16 %", ", -32768", "icmp ne i16 %"],
    );
}

#[test]
fn two_non_negative_tests_anded_are_one_test_of_the_or() {
    check(
        AB,
        "  %x = icmp sgt i16 %a, -1\n  %y = icmp sgt i16 %b, -1\n  %r = and i1 %x, %y\n",
        &["or i16 %a, %b", "and i16 %", ", -32768", "icmp eq i16 %"],
    );
}

#[test]
fn two_zero_tests_anded_are_one_test_of_the_or() {
    check(
        AB,
        "  %x = icmp eq i16 %a, 0\n  %y = icmp eq i16 %b, 0\n  %r = and i1 %x, %y\n",
        &["or i16 %a, %b", "icmp eq i16 %", ", 0"],
    );
}

#[test]
fn two_all_ones_tests_ored_are_one_test_of_the_and() {
    check(
        AB,
        "  %x = icmp ne i16 %a, -1\n  %y = icmp ne i16 %b, -1\n  %r = or i1 %x, %y\n",
        &["and i16 %a, %b", "icmp ne i16 %", ", -1"],
    );
}

#[test]
fn masks_of_zero_tests_merge() {
    check(
        "i16 %a",
        "  %m1 = and i16 %a, 3\n  %m2 = and i16 %a, 12\n  %x = icmp eq i16 %m1, 0\n  %y = icmp eq i16 %m2, 0\n  %r = and i1 %x, %y\n",
        &["and i16 %a, 15", "icmp eq i16 %", ", 0"],
    );
}

#[test]
fn masks_of_all_ones_tests_merge() {
    check(
        "i16 %a",
        "  %m1 = and i16 %a, 3\n  %m2 = and i16 %a, 12\n  %x = icmp eq i16 %m1, 3\n  %y = icmp eq i16 %m2, 12\n  %r = and i1 %x, %y\n",
        &["and i16 %a, 15", "icmp eq i16 %", ", 15"],
    );
}

/// `(A & 12) != 0 && (A & 7) == 1` is `(A & 15) == 9`.
#[test]
fn a_nonzero_test_and_a_mixed_test_merge() {
    check(
        "i16 %a",
        "  %m1 = and i16 %a, 12\n  %m2 = and i16 %a, 7\n  %x = icmp ne i16 %m1, 0\n  %y = icmp eq i16 %m2, 1\n  %r = and i1 %x, %y\n",
        &["and i16 %a, 15", "icmp eq i16 %", ", 9"],
    );
}

#[test]
fn contradicting_mask_tests_are_false() {
    check(
        "i16 %a",
        "  %m1 = and i16 %a, 3\n  %m2 = and i16 %a, 7\n  %x = icmp ne i16 %m1, 0\n  %y = icmp eq i16 %m2, 0\n  %r = and i1 %x, %y\n",
        &["ret i1 false"],
    );
}

#[test]
fn compare_codes_of_one_pair_merge() {
    check(AB, "  %x = icmp slt i16 %a, %b\n  %y = icmp eq i16 %a, %b\n  %r = or i1 %x, %y\n", &["icmp sle i16 %a, %b"]);
}

/// `(X & Y) < 0 || (X | Y) > -1`: the sign bits are equal.
#[test]
fn a_naive_same_sign_test_is_an_xor() {
    check(
        AB,
        "  %n = and i16 %a, %b\n  %o = or i16 %a, %b\n  %x = icmp slt i16 %n, 0\n  %y = icmp sgt i16 %o, -1\n  %r = or i1 %x, %y\n",
        &["xor i16 %a, %b", "icmp sgt i16 %", ", -1"],
    );
}

/// fin.c's `c >= 'a' && c <= 'z'` stayed two compares, a `cset` each, an `and`
/// and a `tst` (`fin_number` +119 instructions over clang): the range of the
/// pair is one `sub` and one compare.
#[test]
fn a_lower_case_range_test_is_one_unsigned_compare() {
    let out = combined("i8 %c", "  %a = icmp sgt i8 %c, 96\n  %b = icmp slt i8 %c, 123\n  %r = and i1 %a, %b\n");
    assert!(out.contains("add i8 %c, -97") && out.contains("icmp ult i8 %") && out.contains(", 26"), "{out}");
    assert_eq!(out.matches("icmp").count(), 1, "{out}");
}

#[test]
fn two_outside_range_tests_ored_are_one_unsigned_compare() {
    let out = combined("i8 %c", "  %a = icmp slt i8 %c, 97\n  %b = icmp sgt i8 %c, 122\n  %r = or i1 %a, %b\n");
    // The wrapped range [123, 97): `getEquivalentICmp` is `(c - 123) u< 230`.
    assert!(out.contains("add i8 %c, -123") && out.contains("icmp ult i8 %") && out.contains(", -26"), "{out}");
    assert_eq!(out.matches("icmp").count(), 1, "{out}");
}

#[test]
fn two_equality_tests_that_differ_in_one_bit_are_one_masked_test() {
    check(
        "i32 %x",
        "  %a = icmp eq i32 %x, 0\n  %b = icmp eq i32 %x, 4\n  %r = or i1 %a, %b\n",
        &["and i32 %x, -5", "icmp eq i32 %", ", 0"],
    );
}

#[test]
fn a_range_with_a_lower_bound_of_zero_and_a_non_negative_end_is_one_unsigned_compare() {
    check(
        "i32 %x, i32 %m",
        "  %n = and i32 %m, 255\n  %a = icmp sge i32 %x, 0\n  %b = icmp slt i32 %x, %n\n  %r = and i1 %a, %b\n",
        &["icmp ult i32 %x, %n"],
    );
}

#[test]
fn a_range_end_that_may_be_negative_is_left_alone() {
    let out =
        combined("i32 %x, i32 %n", "  %a = icmp sge i32 %x, 0\n  %b = icmp slt i32 %x, %n\n  %r = and i1 %a, %b\n");
    assert_eq!(out.matches("icmp").count(), 2, "{out}");
}

/// `c == 37 || c == 38 || c == 33 || c == 35` (t3.c's `suffix` test) left
/// the last two compares apart behind the range: `(A | B) | C` was never
/// regrouped, so `c == 33 || c == 35` stayed two compares and SimplifyCFG
/// found no switch to make.
#[test]
fn an_or_chain_of_equalities_regroups_to_fold_the_last_pair() {
    let out = combined(
        "i8 %c",
        "  %a = icmp eq i8 %c, 37\n  %b = icmp eq i8 %c, 38\n  %o1 = or i1 %a, %b\n  %d = icmp eq i8 %c, 33\n  %o2 = or i1 %o1, %d\n  %e = icmp eq i8 %c, 35\n  %r = or i1 %o2, %e\n",
    );
    assert!(out.contains("and i8 %c, -3") && out.contains("icmp ult i8 %"), "{out}");
    assert_eq!(out.matches("icmp").count(), 2, "{out}");
}
