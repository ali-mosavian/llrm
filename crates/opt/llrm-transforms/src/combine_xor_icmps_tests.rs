use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitXor`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Binary(BinaryOp::Xor) {
        crate::combine_xor::visit_xor(c, inst)
    } else {
        Visit::None
    }
}

/// `@f(params)` after `visitXor` alone; `body` leaves an `i32 %r`.
fn combined(
    params: &str,
    body: &str,
) -> String {
    let mut module = parsed(&format!("define i32 @f({params}) {{\nb0:\n{body}  ret i32 %r\n}}\n"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// `body` must come out with each of `wanted` and none of `unwanted`, the
/// shape pinned `opt -passes=instcombine` gives.
fn check(
    params: &str,
    body: &str,
    wanted: &[&str],
    unwanted: &[&str],
) {
    let out = combined(params, body);
    for one in wanted {
        assert!(out.contains(one), "`{one}` missing\n{out}");
    }
    for one in unwanted {
        assert!(!out.contains(one), "`{one}` present\n{out}");
    }
}

const XY: &str = "i32 %x, i32 %y";

/// Two sign tests xor'd stayed two compares and an `xor i1` (QuickrBasic
/// `fuse_code`: `global(a) != global(b)` cost two `cmn`/`cset` pairs where the
/// pin has one `eor` and a sign test).
#[test]
fn an_xor_of_two_sign_tests_is_a_sign_test_of_the_xor() {
    check(
        XY,
        "  %a = icmp sgt i32 %x, -1\n  %b = icmp sgt i32 %y, -1\n  %v = xor i1 %a, %b\n  %r = zext i1 %v to i32\n",
        &["xor i32 %x, %y", "icmp slt i32 %", ", 0"],
        &["xor i1"],
    );
    check(
        XY,
        "  %a = icmp slt i32 %x, 0\n  %b = icmp slt i32 %y, 0\n  %v = xor i1 %a, %b\n  %r = zext i1 %v to i32\n",
        &["xor i32 %x, %y", "icmp slt i32 %", ", 0"],
        &["xor i1"],
    );
}

#[test]
fn an_xor_of_a_sign_test_and_its_complement_is_a_not_negative_test() {
    check(
        XY,
        "  %a = icmp slt i32 %x, 0\n  %b = icmp sgt i32 %y, -1\n  %v = xor i1 %a, %b\n  %r = zext i1 %v to i32\n",
        &["xor i32 %x, %y", "icmp sgt i32 %", ", -1"],
        &["xor i1"],
    );
}

/// `a < b` ^ `a == b` is the compare of code 4 ^ 2: `a <= b` is wrong (they
/// are exclusive, so the xor is their `or`).
#[test]
fn an_xor_of_two_compares_of_the_same_operands_merges_by_code() {
    check(
        XY,
        "  %a = icmp slt i32 %x, %y\n  %b = icmp eq i32 %x, %y\n  %v = xor i1 %a, %b\n  %r = zext i1 %v to i32\n",
        &["icmp sle i32 %x, %y"],
        &["xor i1"],
    );
}

/// `x < 5` ^ `x < 10` is `5 <= x < 10`.
#[test]
fn an_xor_of_two_ranges_of_one_value_is_one_range_test() {
    check(
        "i32 %x",
        "  %a = icmp ult i32 %x, 5\n  %b = icmp ult i32 %x, 10\n  %v = xor i1 %a, %b\n  %r = zext i1 %v to i32\n",
        &["add i32 %x, -5", "icmp ult i32 %", ", 5"],
        &["xor i1"],
    );
}

#[test]
fn an_xor_of_two_bit_tests_under_one_power_of_two_tests_the_xor() {
    check(
        XY,
        "  %p = and i32 %x, 8\n  %q = and i32 %y, 8\n  %a = icmp eq i32 %p, 0\n  %b = icmp eq i32 %q, 0\n  %v = xor i1 %a, %b\n  %r = zext i1 %v to i32\n",
        &["xor i32 %x, %y", "and i32 %", ", 8", "icmp ne i32 %"],
        &["xor i1"],
    );
}

/// `(x u< y) ^ ((x - y) != 0)`: the `or` is the second compare and the `and`
/// the first, so the xor is `((x - y) != 0) & (x u>= y)`; the first is used
/// again, so it keeps a `not` for its other users.
#[test]
fn a_xor_whose_or_and_and_fold_to_the_two_compares_is_an_and_with_one_inverted() {
    check(
        "i32 %x, i32 %y",
        "  %a = icmp ult i32 %x, %y\n  %d = sub i32 %x, %y\n  %b = icmp ne i32 %d, 0\n  %v = xor i1 %a, %b\n  %e = select i1 %a, i32 1, i32 2\n  %f = zext i1 %b to i32\n  %w = zext i1 %v to i32\n  %g = add i32 %e, %f\n  %r = add i32 %g, %w\n",
        &["and i1 %a, %b", "icmp uge i32 %x, %y", "xor i1 %a, true"],
        &["xor i1 %a, %b"],
    );
}
