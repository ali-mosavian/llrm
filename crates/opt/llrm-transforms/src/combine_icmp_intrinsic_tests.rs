//! `foldICmpIntrinsicWithConstant` against `opt -passes=instcombine` on the
//! same text (pin d1106deb71cc).

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

/// `visitICmpInst` and `visitSelectInst`, through the driver.
fn visit_both(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    match c.function.instruction(inst).opcode {
        Opcode::ICmp(_) => visit_icmp(c, inst),
        Opcode::Select => crate::combine::visit(c, inst),
        _ => Visit::None,
    }
}

/// `@f(params)` returning i1 after `visitICmpInst` alone, with `decls` the
/// intrinsics `body` calls.
fn folded(
    params: &str,
    decls: &str,
    body: &str,
) -> String {
    let mut module = parsed(&format!("{decls}\ndefine i1 @f({params}) {{\nb0:\n{body}  ret i1 %r\n}}\n"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// Some line of `out` is a call of `intrinsic` (its declaration stays).
fn calls(
    out: &str,
    intrinsic: &str,
) -> bool {
    out.lines().any(|line| line.contains("call ") && line.contains(intrinsic))
}

/// `out` holds each of `wanted` and no call of `intrinsic`.
fn check(
    params: &str,
    decls: &str,
    body: &str,
    intrinsic: &str,
    wanted: &[&str],
) {
    let out = folded(params, decls, body);
    for one in wanted {
        assert!(out.contains(one), "`{one}` missing\n{out}");
    }
    // The compare no longer reads the call; the dead call is left for `dce`.
    assert!(
        !out.lines().any(|line| line.contains("icmp") && line.contains("%c")),
        "a compare still reads the {intrinsic} call\n{out}"
    );
}

/// `out` is the same as `body` alone: the pin leaves it.
fn unchanged(
    params: &str,
    decls: &str,
    body: &str,
    intrinsic: &str,
) {
    let out = folded(params, decls, body);
    assert!(calls(&out, intrinsic), "the call of `{intrinsic}` is gone\n{out}");
}

const CTPOP: &str = "declare i32 @llvm.ctpop.i32(i32)";
const CTPOP64: &str = "declare i64 @llvm.ctpop.i64(i64)";
const CTLZ: &str = "declare i32 @llvm.ctlz.i32(i32, i1 immarg)";
const CTTZ: &str = "declare i32 @llvm.cttz.i32(i32, i1 immarg)";
const ABS: &str = "declare i32 @llvm.abs.i32(i32, i1 immarg)";

/// wasm `ones64` after the popcount idiom: `select (ctpop x == 0), 0, trunc
/// ctpop` kept the compare on the count (15 B against the pin's 7), where the
/// pin compares `x` itself.
#[test]
fn a_ctpop_compared_with_zero_compares_its_input() {
    check(
        "i64 %x",
        CTPOP64,
        "  %c = call i64 @llvm.ctpop.i64(i64 %x)\n  %r = icmp eq i64 %c, 0\n",
        "llvm.ctpop",
        &["icmp eq i64 %x, 0"],
    );
    check(
        "i32 %x",
        CTPOP,
        "  %c = call i32 @llvm.ctpop.i32(i32 %x)\n  %r = icmp ne i32 %c, 32\n",
        "llvm.ctpop",
        &["icmp ne i32 %x, -1"],
    );
}

/// `ctpop(x) != 5` has no fold: the call stays.
#[test]
fn a_ctpop_compared_with_another_constant_is_left() {
    unchanged("i32 %x", CTPOP, "  %c = call i32 @llvm.ctpop.i32(i32 %x)\n  %r = icmp eq i32 %c, 5\n", "llvm.ctpop");
}

/// `ctlz`/`cttz` equal to the bit width are `x == 0`; equal to a smaller
/// count, one use, a masked compare.
#[test]
fn a_count_of_zeros_compared_with_a_constant_is_a_mask_test() {
    check(
        "i32 %x",
        CTLZ,
        "  %c = call i32 @llvm.ctlz.i32(i32 %x, i1 false)\n  %r = icmp eq i32 %c, 32\n",
        "llvm.ctlz",
        &["icmp eq i32 %x, 0"],
    );
    check(
        "i32 %x",
        CTTZ,
        "  %c = call i32 @llvm.cttz.i32(i32 %x, i1 false)\n  %r = icmp eq i32 %c, 3\n",
        "llvm.cttz",
        &["and i32 %x, 15", "icmp eq i32", ", 8"],
    );
    check(
        "i32 %x",
        CTLZ,
        "  %c = call i32 @llvm.ctlz.i32(i32 %x, i1 false)\n  %r = icmp eq i32 %c, 3\n",
        "llvm.ctlz",
        &["and i32 %x, -268435456", "icmp eq i32", ", 268435456"],
    );
}

/// A relational compare of a count with a constant is a compare of the input.
#[test]
fn a_count_compared_by_order_is_a_compare_of_the_input() {
    check(
        "i32 %x",
        CTPOP,
        "  %c = call i32 @llvm.ctpop.i32(i32 %x)\n  %r = icmp ugt i32 %c, 31\n",
        "llvm.ctpop",
        &["icmp eq i32 %x, -1"],
    );
    check(
        "i32 %x",
        CTPOP,
        "  %c = call i32 @llvm.ctpop.i32(i32 %x)\n  %r = icmp ult i32 %c, 32\n",
        "llvm.ctpop",
        &["icmp ne i32 %x, -1"],
    );
    check(
        "i32 %x",
        CTLZ,
        "  %c = call i32 @llvm.ctlz.i32(i32 %x, i1 false)\n  %r = icmp ugt i32 %c, 3\n",
        "llvm.ctlz",
        &["icmp ult i32 %x, 268435456"],
    );
    check(
        "i32 %x",
        CTLZ,
        "  %c = call i32 @llvm.ctlz.i32(i32 %x, i1 false)\n  %r = icmp ult i32 %c, 3\n",
        "llvm.ctlz",
        &["icmp ugt i32 %x, 536870911"],
    );
    check(
        "i32 %x",
        CTTZ,
        "  %c = call i32 @llvm.cttz.i32(i32 %x, i1 false)\n  %r = icmp ugt i32 %c, 3\n",
        "llvm.cttz",
        &["and i32 %x, 15", "icmp eq i32", ", 0"],
    );
    check(
        "i32 %x",
        CTTZ,
        "  %c = call i32 @llvm.cttz.i32(i32 %x, i1 false)\n  %r = icmp ult i32 %c, 3\n",
        "llvm.cttz",
        &["and i32 %x, 7", "icmp ne i32", ", 0"],
    );
}

/// `cttz` with a second user is not folded by order (it would add an `and`).
#[test]
fn a_count_with_two_users_is_not_compared_by_order() {
    let out = folded(
        "i32 %x",
        &format!("{CTTZ}\ndeclare void @use(i32)"),
        "  %c = call i32 @llvm.cttz.i32(i32 %x, i1 false)\n  call void @use(i32 %c)\n  %r = icmp ugt i32 %c, 3\n",
    );
    assert!(calls(&out, "llvm.cttz"), "{out}");
    assert!(out.contains("ugt i32 %c, 3"), "{out}");
}

/// `bswap`, `bitreverse` and `abs` against a constant compare the input with
/// the transformed constant.
#[test]
fn a_byte_or_bit_permutation_compared_with_a_constant_moves_to_the_constant() {
    check(
        "i32 %x",
        "declare i32 @llvm.bswap.i32(i32)",
        "  %c = call i32 @llvm.bswap.i32(i32 %x)\n  %r = icmp eq i32 %c, 287454020\n",
        "llvm.bswap",
        &["icmp eq i32 %x, 1144201745"],
    );
    check(
        "i32 %x",
        "declare i32 @llvm.bitreverse.i32(i32)",
        "  %c = call i32 @llvm.bitreverse.i32(i32 %x)\n  %r = icmp ne i32 %c, 1\n",
        "llvm.bitreverse",
        &["icmp ne i32 %x, -2147483648"],
    );
    check(
        "i32 %x",
        ABS,
        "  %c = call i32 @llvm.abs.i32(i32 %x, i1 false)\n  %r = icmp eq i32 %c, 0\n",
        "llvm.abs",
        &["icmp eq i32 %x, 0"],
    );
}

/// `abs(x) u> C` and `u< C` with one use.
#[test]
fn abs_compared_by_order_is_a_biased_compare() {
    check(
        "i32 %x",
        ABS,
        "  %c = call i32 @llvm.abs.i32(i32 %x, i1 false)\n  %r = icmp ugt i32 %c, 5\n",
        "llvm.abs",
        &["add i32 %x, -6", "icmp ult i32", ", -11"],
    );
    check(
        "i32 %x",
        ABS,
        "  %c = call i32 @llvm.abs.i32(i32 %x, i1 false)\n  %r = icmp ult i32 %c, 5\n",
        "llvm.abs",
        &["add i32 %x, 4", "icmp ult i32", ", 9"],
    );
}

/// A rotate by a constant compared with a constant rotates the constant the
/// other way; a rotate compared with 0 or -1 is its input.
#[test]
fn a_rotate_compared_with_a_constant_rotates_the_constant_back() {
    check(
        "i32 %x",
        "declare i32 @llvm.fshl.i32(i32, i32, i32)",
        "  %c = call i32 @llvm.fshl.i32(i32 %x, i32 %x, i32 8)\n  %r = icmp eq i32 %c, 305419896\n",
        "llvm.fshl",
        &["icmp eq i32 %x, 2014458966"],
    );
    check(
        "i32 %x, i32 %n",
        "declare i32 @llvm.fshl.i32(i32, i32, i32)",
        "  %c = call i32 @llvm.fshl.i32(i32 %x, i32 %x, i32 %n)\n  %r = icmp eq i32 %c, -1\n",
        "llvm.fshl",
        &["icmp eq i32 %x, -1"],
    );
}

/// Saturating arithmetic against zero.
#[test]
fn saturating_arithmetic_compared_with_zero_compares_its_operands() {
    check(
        "i32 %a, i32 %b",
        "declare i32 @llvm.umax.i32(i32, i32)",
        "  %c = call i32 @llvm.umax.i32(i32 %a, i32 %b)\n  %r = icmp eq i32 %c, 0\n",
        "llvm.umax",
        &["or i32 %a, %b", "icmp eq i32"],
    );
    check(
        "i32 %a, i32 %b",
        "declare i32 @llvm.uadd.sat.i32(i32, i32)",
        "  %c = call i32 @llvm.uadd.sat.i32(i32 %a, i32 %b)\n  %r = icmp ne i32 %c, 0\n",
        "llvm.uadd.sat",
        &["or i32 %a, %b", "icmp ne i32"],
    );
    check(
        "i32 %a, i32 %b",
        "declare i32 @llvm.ssub.sat.i32(i32, i32)",
        "  %c = call i32 @llvm.ssub.sat.i32(i32 %a, i32 %b)\n  %r = icmp eq i32 %c, 0\n",
        "llvm.ssub.sat",
        &["icmp eq i32 %a, %b"],
    );
    check(
        "i32 %a, i32 %b",
        "declare i32 @llvm.usub.sat.i32(i32, i32)",
        "  %c = call i32 @llvm.usub.sat.i32(i32 %a, i32 %b)\n  %r = icmp eq i32 %c, 0\n",
        "llvm.usub.sat",
        &["icmp ule i32 %a, %b"],
    );
    check(
        "i32 %a, i32 %b",
        "declare i32 @llvm.ssub.sat.i32(i32, i32)",
        "  %c = call i32 @llvm.ssub.sat.i32(i32 %a, i32 %b)\n  %r = icmp slt i32 %c, 1\n",
        "llvm.ssub.sat",
        &["icmp sle i32 %a, %b"],
    );
}

/// A compare of `uadd.sat`/`usub.sat` with a constant, one use, is one
/// compare of the operand.
#[test]
fn a_saturating_add_or_subtract_compared_with_a_constant_is_one_compare() {
    check(
        "i32 %x",
        "declare i32 @llvm.uadd.sat.i32(i32, i32)",
        "  %c = call i32 @llvm.uadd.sat.i32(i32 %x, i32 5)\n  %r = icmp ult i32 %c, 10\n",
        "llvm.uadd.sat",
        &["icmp ult i32 %x, 5"],
    );
    check(
        "i32 %x",
        "declare i32 @llvm.usub.sat.i32(i32, i32)",
        "  %c = call i32 @llvm.usub.sat.i32(i32 %x, i32 5)\n  %r = icmp ugt i32 %c, 3\n",
        "llvm.usub.sat",
        &["icmp ugt i32 %x, 8"],
    );
}

/// `ctpop` of a value with a known set bit, tested for a power of two, masks
/// that bit out.
#[test]
fn a_power_of_two_test_of_a_value_with_a_known_bit_is_a_mask_test() {
    check(
        "i32 %x",
        CTPOP,
        "  %o = or i32 %x, 4\n  %c = call i32 @llvm.ctpop.i32(i32 %o)\n  %r = icmp ult i32 %c, 2\n",
        "llvm.ctpop",
        &["and i32 %x, -5", "icmp eq i32", ", 0"],
    );
}

/// `ucmp`/`scmp` against a small constant compare the operands.
#[test]
fn a_three_way_compare_tested_against_a_small_constant_compares_its_operands() {
    check(
        "i32 %a, i32 %b",
        "declare i8 @llvm.ucmp.i8.i32(i32, i32)",
        "  %c = call i8 @llvm.ucmp.i8.i32(i32 %a, i32 %b)\n  %r = icmp eq i8 %c, 1\n",
        "llvm.ucmp",
        &["icmp ugt i32 %a, %b"],
    );
    check(
        "i32 %a, i32 %b",
        "declare i8 @llvm.scmp.i8.i32(i32, i32)",
        "  %c = call i8 @llvm.scmp.i8.i32(i32 %a, i32 %b)\n  %r = icmp slt i8 %c, 1\n",
        "llvm.scmp",
        &["icmp sle i32 %a, %b"],
    );
}

/// Two calls of one intrinsic compared for equality compare their inputs.
#[test]
fn two_byte_swaps_or_equal_rotates_compared_for_equality_compare_their_inputs() {
    check(
        "i32 %a, i32 %b",
        "declare i32 @llvm.bswap.i32(i32)",
        "  %p = call i32 @llvm.bswap.i32(i32 %a)\n  %q = call i32 @llvm.bswap.i32(i32 %b)\n  %r = icmp eq i32 %p, %q\n",
        "llvm.bswap",
        &["icmp eq i32 %a, %b"],
    );
    check(
        "i32 %a, i32 %b",
        "declare i32 @llvm.fshl.i32(i32, i32, i32)",
        "  %p = call i32 @llvm.fshl.i32(i32 %a, i32 %a, i32 3)\n  %q = call i32 @llvm.fshl.i32(i32 %b, i32 %b, i32 3)\n  %r = icmp ne i32 %p, %q\n",
        "llvm.fshl",
        &["icmp ne i32 %a, %b"],
    );
}

/// Rotates by different amounts become one rotate by the difference.
#[test]
fn rotates_by_different_amounts_compared_for_equality_are_one_rotate() {
    let out = folded(
        "i32 %a, i32 %b",
        "declare i32 @llvm.fshl.i32(i32, i32, i32)",
        "  %p = call i32 @llvm.fshl.i32(i32 %a, i32 %a, i32 3)\n  %q = call i32 @llvm.fshl.i32(i32 %b, i32 %b, i32 5)\n  %r = icmp eq i32 %p, %q\n",
    );
    assert!(out.contains("call i32 @llvm.fshl.i32(i32 %a, i32 %a, i32 -2)"), "{out}");
    assert!(out.contains("icmp eq i32 %b,"), "{out}");
}

/// The count guarded against zero (`x == 0 ? 0 : ctpop(x)`, what the popcount
/// loop idiom leaves) is the count alone: the first head of this fold compared
/// `x` instead of the count and the select stayed (wasm `ones32` +3 B).
#[test]
fn a_ctpop_guarded_against_zero_is_the_count_alone() {
    for (cmp, arms) in [("eq", "i32 0, i32 %c"), ("ne", "i32 %c, i32 0")] {
        let mut module = parsed(&format!(
            "{CTPOP}\ndefine i32 @f(i32 %x) {{\nb0:\n  %c = call i32 @llvm.ctpop.i32(i32 %x)\n  %z = icmp {cmp} i32 %x, 0\n  %r = select i1 %z, {arms}\n  ret i32 %r\n}}\n"
        ));
        let globals = module.globals.clone();
        let metadata = module.metadata.clone();
        let (context, function) = module.function_mut("f").expect("@f");
        combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit_both);
        let out = printed(&module);
        assert!(!out.contains("select"), "{cmp}: the select is left\n{out}");
    }
}
