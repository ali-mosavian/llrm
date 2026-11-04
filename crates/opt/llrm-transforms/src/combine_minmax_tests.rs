//! `combine_minmax`: a min or max of extended operands is narrow.

use llrm_mir::datalayout::DataLayout;

use crate::combine::combine_with_declared;
use crate::testing::{parsed, printed};

fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with_declared(
        context,
        &globals,
        &metadata,
        &DataLayout::default(),
        function,
        |c, inst| {
            if matches!(
                c.function.instruction(inst).opcode,
                llrm_mir::opcode::Opcode::Call(_)
            ) {
                c.visit_call(inst)
            } else {
                crate::combine::Visit::None
            }
        },
        &mut declared,
    );
    declared.place(&mut module).unwrap();
    printed(&module)
}

/// The closed form of `sum i for i < min(n, 10)` took `umin.i33` of a
/// `zext i16`, a second 64-bit compare beside the 16-bit one the same value
/// already had: 107 bytes at m16 -Os where main has 35.
#[test]
fn a_umin_of_a_zext_and_a_constant_is_the_zext_of_a_narrow_umin() {
    let after = combined(
        "declare i33 @llvm.umin.i33(i33, i33)

define i33 @f(i16 %x) {
b0:
  %e = zext i16 %x to i33
  %m = call i33 @llvm.umin.i33(i33 %e, i33 9)
  ret i33 %m
}
",
    );
    assert!(after.contains("@llvm.umin.i16(i16 %x, i16 9)") && after.contains("zext i16 %"), "{after}");
    assert!(!after.contains("@llvm.umin.i33(") || after.contains("declare i33"), "{after}");
    assert!(!after.contains("call i33"), "{after}");
}

/// A constant past the narrow type is not narrowed, and the range of the
/// `zext` (below 2^16) decides the min: pinned `opt -passes=instcombine`.
#[test]
fn a_constant_the_narrow_type_cannot_hold_is_decided_by_the_range() {
    let after = combined(
        "declare i32 @llvm.umin.i32(i32, i32)

define i32 @f(i16 %x) {
b0:
  %e = zext i16 %x to i32
  %m = call i32 @llvm.umin.i32(i32 %e, i32 70000)
  ret i32 %m
}
",
    );
    assert!(!after.contains("call i32 @llvm.umin.i32(") && after.contains("zext i16 %x to i32"), "{after}");
}

/// Two extensions of one type, and the signed pair.
#[test]
fn two_extensions_of_one_type_narrow_and_a_smax_takes_sext() {
    let after = combined(
        "declare i32 @llvm.smax.i32(i32, i32)

define i32 @f(i8 %x, i8 %y) {
b0:
  %a = sext i8 %x to i32
  %b = sext i8 %y to i32
  %m = call i32 @llvm.smax.i32(i32 %a, i32 %b)
  ret i32 %m
}
",
    );
    assert!(after.contains("@llvm.smax.i8(i8 %x, i8 %y)") && after.contains("sext i8 %"), "{after}");
}

/// demo-deedlines COPPER at m16 -Os: the trip-count selects left `umin.i1`
/// and `smax.i1` calls (`and` in the pin), +74 lines of the listing. The four
/// min/max of two `i1` are `and` for `umin` and `smax`, `or` for `umax` and
/// `smin`.
#[test]
fn a_min_or_max_of_two_bits_is_an_and_or_an_or() {
    let after = combined(
        "declare i1 @llvm.umin.i1(i1, i1)
declare i1 @llvm.umax.i1(i1, i1)
declare i1 @llvm.smin.i1(i1, i1)
declare i1 @llvm.smax.i1(i1, i1)

define i16 @f(i1 %a, i1 %b) {
b0:
  %m = call i1 @llvm.umin.i1(i1 %a, i1 %b)
  %n = call i1 @llvm.umax.i1(i1 %a, i1 %b)
  %o = call i1 @llvm.smin.i1(i1 %a, i1 %b)
  %p = call i1 @llvm.smax.i1(i1 %a, i1 %b)
  %x = zext i1 %m to i16
  %y = zext i1 %n to i16
  %z = zext i1 %o to i16
  %w = zext i1 %p to i16
  %s = add i16 %x, %y
  %t = add i16 %z, %w
  %r = add i16 %s, %t
  ret i16 %r
}
",
    );
    assert!(!after.contains("call i1"), "{after}");
    assert_eq!(after.matches("and i1 %a, %b").count(), 2, "{after}");
    assert_eq!(after.matches("or i1 %a, %b").count(), 2, "{after}");
}

const MINMAX_DECLARES: &str = "declare i32 @llvm.umin.i32(i32, i32)\ndeclare i32 @llvm.umax.i32(i32, i32)\ndeclare i32 @llvm.smin.i32(i32, i32)\ndeclare i32 @llvm.smax.i32(i32, i32)\n";

fn minmax(body: &str) -> String {
    combined(&format!("{MINMAX_DECLARES}define i32 @f(i32 %x, i32 %y) {{\n{body}}}\n"))
}

/// Pinned `opt -passes=instcombine`: `umin(x, 1)` is `zext (x != 0)`. Missing,
/// the intrinsic call stayed.
#[test]
fn a_umin_with_one_is_a_zext_of_a_nonzero_test() {
    let out = minmax("  %r = call i32 @llvm.umin.i32(i32 %x, i32 1)\n  ret i32 %r\n");
    assert!(out.contains("icmp ne i32 %x, 0") && out.contains("zext i1"), "{out}");
    assert!(!out.contains("call i32 @llvm.umin"), "{out}");
}

/// `umax(nuw_shl(x, 3), x + 1)` is `x == 0 ? 1 : x << 3`.
#[test]
fn a_umax_of_a_nuw_shift_and_its_operand_plus_one_is_a_select_on_zero() {
    let out = minmax(
        "  %a = shl nuw i32 %x, 3\n  %b = add i32 %x, 1\n  %r = call i32 @llvm.umax.i32(i32 %a, i32 %b)\n  ret i32 %r\n",
    );
    assert!(out.contains("icmp eq i32 %x, 0") && out.contains("select i1"), "{out}");
    assert!(!out.contains("call i32 @llvm.umax"), "{out}");
}

/// `smax(smin(x, 10), 3)` is `smin(smax(x, 3), 10)`: the clamp's canonical
/// form.
#[test]
fn a_clamp_is_written_max_first() {
    let out = minmax(
        "  %m = call i32 @llvm.smin.i32(i32 %x, i32 10)\n  %r = call i32 @llvm.smax.i32(i32 %m, i32 3)\n  ret i32 %r\n",
    );
    let max = out.find("llvm.smax.i32(i32 %x, i32 3)").unwrap_or_else(|| panic!("{out}"));
    let min = out.find("llvm.smin.i32(i32 %1, i32 10)").unwrap_or_else(|| panic!("{out}"));
    assert!(max < min, "{out}");
}

/// Not a clamp: `MinC s> MaxC` fails, so the form stays.
#[test]
fn a_smax_of_a_smin_below_the_bound_is_left_alone() {
    let out = minmax(
        "  %m = call i32 @llvm.smin.i32(i32 %x, i32 3)\n  %r = call i32 @llvm.smax.i32(i32 %m, i32 10)\n  ret i32 %r\n",
    );
    assert!(out.contains("llvm.smin.i32(i32 %x, i32 3)"), "{out}");
}

/// `smin(smax(x, -1), 1)` is `scmp(x, 0)`.
#[test]
fn a_clamp_to_minus_one_and_one_is_a_three_way_compare() {
    let out = minmax(
        "  %m = call i32 @llvm.smax.i32(i32 %x, i32 -1)\n  %r = call i32 @llvm.smin.i32(i32 %m, i32 1)\n  ret i32 %r\n",
    );
    assert!(out.contains("llvm.scmp.i32.i32(i32 %x, i32 0)"), "{out}");
}

/// `smax(neg nsw x, neg nsw y)` is `neg nsw (smin x, y)`.
#[test]
fn a_smax_of_two_nsw_negations_is_the_negation_of_the_smin() {
    let out = minmax(
        "  %a = sub nsw i32 0, %x\n  %b = sub nsw i32 0, %y\n  %r = call i32 @llvm.smax.i32(i32 %a, i32 %b)\n  ret i32 %r\n",
    );
    assert!(out.contains("llvm.smin.i32(i32 %x, i32 %y)") && out.contains("sub nsw i32 0, %1"), "{out}");
}

/// `smax(x, -x)` is `abs(x)`, with the nsw of the negation as the poison flag;
/// `umax(x, -x)` is `-abs(x)`.
#[test]
fn a_max_of_a_value_and_its_negation_is_an_abs() {
    let out = minmax("  %n = sub nsw i32 0, %x\n  %r = call i32 @llvm.smax.i32(i32 %x, i32 %n)\n  ret i32 %r\n");
    assert!(out.contains("llvm.abs.i32(i32 %x, i1 true)"), "{out}");
    let out = minmax("  %n = sub i32 0, %x\n  %r = call i32 @llvm.umax.i32(i32 %x, i32 %n)\n  ret i32 %r\n");
    assert!(out.contains("llvm.abs.i32(i32 %x, i1 false)") && out.contains("sub i32 0, %1"), "{out}");
}

/// A `umin` against a constant the left's range already clears is the left.
#[test]
fn a_min_against_a_constant_the_range_of_the_left_decides_is_the_left() {
    let out = combined(&format!(
        "{MINMAX_DECLARES}define i32 @f(i8 %x) {{\n  %z = zext i8 %x to i32\n  %r = call i32 @llvm.umin.i32(i32 %z, i32 300)\n  ret i32 %r\n}}\n"
    ));
    assert!(!out.contains("call i32 @llvm.umin") && out.contains("zext i8 %x to i32"), "{out}");
}

fn compare(
    declares: &str,
    params: &str,
    body: &str,
    ret: &str,
) -> String {
    combined(&format!("{declares}define {ret} @f({params}) {{\n{body}}}\n"))
}

const CMP_DECLARES: &str = "declare i8 @llvm.scmp.i8.i64(i64, i64)\ndeclare i8 @llvm.scmp.i8.i32(i32, i32)\n";

/// Pinned `opt -passes=instcombine`: `scmp (sext X), (sext Y)` is `scmp X, Y`;
/// the zext pair is `ucmp X, Y`. Missing, the compare stayed 64 bits wide.
#[test]
fn a_compare_of_two_extensions_is_a_compare_of_the_narrow_operands() {
    let out = compare(
        CMP_DECLARES,
        "i16 %x, i16 %y",
        "  %a = sext i16 %x to i64\n  %b = sext i16 %y to i64\n  %r = call i8 @llvm.scmp.i8.i64(i64 %a, i64 %b)\n  ret i8 %r\n",
        "i8",
    );
    assert!(out.contains("call i8 @llvm.scmp.i8.i16(i16 %x, i16 %y)"), "{out}");
    let out = compare(
        CMP_DECLARES,
        "i16 %x, i16 %y",
        "  %a = zext i16 %x to i64\n  %b = zext i16 %y to i64\n  %r = call i8 @llvm.scmp.i8.i64(i64 %a, i64 %b)\n  ret i8 %r\n",
        "i8",
    );
    assert!(out.contains("call i8 @llvm.ucmp.i8.i16(i16 %x, i16 %y)"), "{out}");
}

/// A constant narrows with the extension of its other operand.
#[test]
fn a_compare_of_an_extension_and_a_constant_narrows_the_constant() {
    let out = compare(
        CMP_DECLARES,
        "i16 %x",
        "  %a = zext i16 %x to i64\n  %r = call i8 @llvm.scmp.i8.i64(i64 %a, i64 7)\n  ret i8 %r\n",
        "i8",
    );
    assert!(out.contains("call i8 @llvm.ucmp.i8.i16(i16 %x, i16 7)"), "{out}");
}

/// Different extensions do not combine.
#[test]
fn a_compare_of_a_sext_and_a_zext_stays_wide() {
    let out = compare(
        CMP_DECLARES,
        "i16 %x, i8 %y",
        "  %a = sext i16 %x to i32\n  %b = zext i8 %y to i32\n  %r = call i8 @llvm.scmp.i8.i32(i32 %a, i32 %b)\n  ret i8 %r\n",
        "i8",
    );
    assert!(out.contains("call i8 @llvm.scmp.i8.i32(i32 %a, i32 %b)"), "{out}");
}

/// `scmp(nsw (sub X, Y), 0)` is `scmp(X, Y)`.
#[test]
fn a_compare_of_an_nsw_difference_with_zero_is_a_compare_of_the_operands() {
    let out = compare(
        "declare i8 @llvm.scmp.i8.i32(i32, i32)\n",
        "i32 %x, i32 %y",
        "  %d = sub nsw i32 %x, %y\n  %r = call i8 @llvm.scmp.i8.i32(i32 %d, i32 0)\n  ret i8 %r\n",
        "i8",
    );
    assert!(out.contains("call i8 @llvm.scmp.i8.i32(i32 %x, i32 %y)"), "{out}");
}

/// A value known to be -1, 0 or 1 compares with zero as itself.
#[test]
fn a_compare_with_zero_of_a_value_in_minus_one_to_one_is_the_value() {
    let out = compare(
        "declare i8 @llvm.scmp.i8.i32(i32, i32)\n",
        "i32 %x",
        "  %s = and i32 %x, 1\n  %r = call i8 @llvm.scmp.i8.i32(i32 %s, i32 0)\n  ret i8 %r\n",
        "i8",
    );
    assert!(!out.contains("call i8 @llvm.scmp"), "{out}");
}
