use llrm_mir::datalayout::DataLayout;

use crate::combine::{combine_with, combine_with_declared, visit};
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8\"\n";

/// `@f(i8 %a, i16 %c, ptr %p)` after the whole InstCombine visit.
fn combined(body: &str) -> String {
    let mut module = parsed(&format!("{HEAD}define i1 @f(i8 %a, i16 %c, ptr %p) {{\nb0:\n{body}}}\n"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// `%a` against `zext (trunc %c)`, `pred` and operand order as given.
fn zext_of_trunc(compare: &str) -> String {
    combined(&format!(
        "  %t = trunc i16 %c to i8\n  %z1 = zext i8 %a to i16\n  %z2 = zext i8 %t to i16\n{compare}  ret i1 %r\n"
    ))
}

/// strchr/memchr/strrchr on the 6502 grew 42 -> 73, 63 -> 81 and 41 -> 59
/// bytes: `visitZExt` turned `zext (trunc c)` into `and c, 255` and the
/// compare stayed at i16, where the pin narrows it (`foldICmpEquality`:
/// `(B & (Pow2C-1)) == zext A` is `A == trunc B`).
#[test]
fn a_masked_value_equal_to_a_zext_is_a_narrow_compare() {
    let out = zext_of_trunc("  %r = icmp eq i16 %z1, %z2\n");
    assert!(out.contains("trunc i16 %c to i8") && out.contains("icmp eq i8 %a, "), "{out}");
    assert!(!out.contains("and i16") && !out.contains("zext"), "{out}");
}

/// `!=` and the other operand order take the same fold.
#[test]
fn not_equal_and_the_other_order_narrow_too() {
    let out = zext_of_trunc("  %r = icmp ne i16 %z2, %z1\n");
    assert!(out.contains("trunc i16 %c to i8") && out.contains("icmp ne i8 %a, "), "{out}");
    assert!(!out.contains("and i16") && !out.contains("zext"), "{out}");
}

/// Control: only equality narrows. `ult` stays as the pin leaves it, the mask
/// and a wide compare.
#[test]
fn an_unsigned_compare_stays_wide() {
    let out = zext_of_trunc("  %r = icmp ult i16 %z1, %z2\n");
    assert!(out.contains("and i16 %c, 255") && out.contains("zext i8 %a to i16"), "{out}");
    assert!(!out.contains("icmp ult i8") && !out.contains("trunc"), "{out}");
}

/// A second user of the extension keeps the `and` for it; the compare still
/// narrows.
#[test]
fn a_zext_with_a_second_user_still_narrows_the_compare() {
    let out = zext_of_trunc("  store i16 %z2, ptr %p\n  %r = icmp eq i16 %z1, %z2\n");
    assert!(out.contains("and i16 %c, 255"), "{out}");
    assert!(out.contains("trunc i16 %c to i8") && out.contains("icmp eq i8 %a, "), "{out}");
}

/// `@f` of `params` after the whole InstCombine visit, with the intrinsics and
/// `@use` declared.
fn run(
    params: &str,
    body: &str,
) -> String {
    let text = format!(
        "{HEAD}declare i16 @llvm.fshr.i16(i16, i16, i16)\ndeclare i16 @llvm.fshl.i16(i16, i16, i16)\ndeclare i16 @llvm.bswap.i16(i16)\ndeclare void @use(i16)\ndefine i1 @f({params}) {{\nb0:\n{body}}}\n"
    );
    let mut module = parsed(&text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with_declared(context, &globals, &metadata, &DataLayout::default(), function, visit, &mut declared);
    declared.place(&mut module).unwrap();
    printed(&module)
}

/// Each of `wanted` must be in the output, as the pinned `opt
/// -passes=instcombine` prints it.
fn check(
    params: &str,
    body: &str,
    wanted: &[&str],
) {
    let out = run(params, body);
    for one in wanted {
        assert!(out.contains(one), "`{one}` missing\n{out}");
    }
}

const AB: &str = "i16 %a, i16 %b";

/// `(A^B) == A` is `B == 0`.
#[test]
fn a_xor_compared_with_its_operand_is_a_zero_test() {
    check(AB, "  %x = xor i16 %a, %b\n  %r = icmp eq i16 %x, %a\n  ret i1 %r\n", &["icmp eq i16 %b, 0"]);
}

/// `A^B != A^D` is `B != D`.
#[test]
fn two_xors_sharing_an_operand_compare_the_others() {
    check(
        "i16 %a, i16 %b, i16 %d",
        "  %x = xor i16 %a, %b\n  %y = xor i16 %a, %d\n  %r = icmp ne i16 %x, %y\n  ret i1 %r\n",
        &["icmp ne i16 %b, %d"],
    );
}

/// `A^5 == C^9` is `(C^A) == 12`.
#[test]
fn two_xors_with_constants_fold_the_constants() {
    check(
        AB,
        "  %x = xor i16 %a, 5\n  %y = xor i16 %b, 9\n  %r = icmp eq i16 %x, %y\n  ret i1 %r\n",
        &["xor i16 %b, %a", ", 12"],
    );
}

/// `(X&Z) == (Y&Z)` is `((X^Y)&Z) == 0`.
#[test]
fn two_ands_with_a_common_mask_are_one_masked_xor() {
    check(
        "i16 %x, i16 %y, i16 %z",
        "  %p = and i16 %x, %z\n  %q = and i16 %y, %z\n  %r = icmp eq i16 %p, %q\n  ret i1 %r\n",
        &["xor i16 %x, %y", "and i16 %", ", %z", "icmp eq i16 %", ", 0"],
    );
}

/// `(X|12) != (Y|12)` is `((X^Y) & ~12) != 0`.
#[test]
fn two_ors_with_a_constant_are_a_masked_xor() {
    check(
        AB,
        "  %p = or i16 %a, 12\n  %q = or i16 %b, 12\n  %r = icmp ne i16 %p, %q\n  ret i1 %r\n",
        &["xor i16 %a, %b", ", -13", "icmp ne i16 %", ", 0"],
    );
}

/// `(A>>3) == (B>>3)` is `(A^B) u< 8`.
#[test]
fn equal_right_shifts_are_an_unsigned_compare_of_the_xor() {
    check(
        AB,
        "  %p = lshr i16 %a, 3\n  %q = lshr i16 %b, 3\n  %r = icmp eq i16 %p, %q\n  ret i1 %r\n",
        &["xor i16 %a, %b", "icmp ult i16 %"],
    );
}

/// `(A<<3) != (B<<3)` is `((A^B) & 8191) != 0`.
#[test]
fn equal_left_shifts_are_a_masked_xor() {
    check(
        AB,
        "  %p = shl i16 %a, 3\n  %q = shl i16 %b, 3\n  %r = icmp ne i16 %p, %q\n  ret i1 %r\n",
        &["xor i16 %a, %b", ", 8191", "icmp ne i16 %", ", 0"],
    );
}

/// `trunc (lshr x, 8) == 7` of a value with other users is a mask compare.
#[test]
fn a_truncated_shift_of_a_shared_value_is_a_mask_compare() {
    check(
        "i16 %x",
        "  call void @use(i16 %x)\n  %s = lshr i16 %x, 8\n  %t = trunc i16 %s to i8\n  %r = icmp eq i8 %t, 7\n  ret i1 %r\n",
        &["and i16 %x, -256", "icmp eq i16 %", ", 1792"],
    );
}

/// The top half and the sign of the low half agreeing is a range test.
#[test]
fn top_bits_all_the_same_is_one_range_check() {
    check(
        "i32 %x",
        "  %lo = trunc i32 %x to i16\n  %s = ashr i16 %lo, 15\n  %h = lshr i32 %x, 16\n  %t = trunc i32 %h to i16\n  %r = icmp eq i16 %s, %t\n  ret i1 %r\n",
        &["add i32 %x, 32768", "icmp ult i32 %", ", 65536"],
    );
}

/// `x & p2 != p2` is `x & p2 == 0` when `p2` is a power of two.
#[test]
fn a_masked_bit_compared_with_the_bit_inverts() {
    check(
        AB,
        "  %p = shl i16 1, %b\n  %m = and i16 %a, %p\n  %r = icmp ne i16 %m, %p\n  ret i1 %r\n",
        &["icmp eq i16 %m, 0"],
    );
}

/// `x == fshr(x, x, n)` is `x == fshl(x, x, n)`.
#[test]
fn a_rotate_right_compared_with_its_operand_is_a_rotate_left() {
    check(
        AB,
        "  %f = call i16 @llvm.fshr.i16(i16 %a, i16 %a, i16 %b)\n  %r = icmp eq i16 %a, %f\n  ret i1 %r\n",
        &["call i16 @llvm.fshl.i16(i16 %a, i16 %a, i16 %b)"],
    );
}

/// `(x^5) == y` is `(x^y) == 5`.
#[test]
fn a_xor_with_a_constant_compared_with_a_value_moves_the_constant() {
    check(
        AB,
        "  %p = xor i16 %a, 5\n  %r = icmp eq i16 %p, %b\n  ret i1 %r\n",
        &["xor i16 %a, %b", "icmp eq i16 %", ", 5"],
    );
}

/// `((x + p2) & p2) == p2` is `(x & p2) == 0` (the pin prints `(p2 & x)`).
#[test]
fn an_and_of_a_sum_with_a_power_of_two_drops_the_sum() {
    check(
        AB,
        "  %p = shl i16 1, %b\n  %s = add i16 %a, %p\n  %m = and i16 %s, %p\n  %r = icmp eq i16 %m, %p\n  ret i1 %r\n",
        &["and i16 %p, %a", "icmp eq i16 %", ", 0"],
    );
}

/// Two byte swaps compare the originals.
#[test]
fn two_byte_swaps_compare_their_operands() {
    check(
        AB,
        "  %x = call i16 @llvm.bswap.i16(i16 %a)\n  %y = call i16 @llvm.bswap.i16(i16 %b)\n  %r = icmp eq i16 %x, %y\n  ret i1 %r\n",
        &["icmp eq i16 %a, %b"],
    );
}

/// `rotl(a, n) == rotl(b, m)` is `b == rotl(a, n - m)`.
#[test]
fn two_rotates_by_different_amounts_are_one_rotate() {
    check(
        "i16 %a, i16 %b, i16 %n, i16 %m",
        "  %x = call i16 @llvm.fshl.i16(i16 %a, i16 %a, i16 %n)\n  %y = call i16 @llvm.fshl.i16(i16 %b, i16 %b, i16 %m)\n  %r = icmp eq i16 %x, %y\n  ret i1 %r\n",
        &["sub i16 %n, %m", "call i16 @llvm.fshl.i16(i16 %a, i16 %a, i16 %", "icmp eq i16 %b, %"],
    );
}

/// `foldICmpEqualityWithOffset`: `(a - k) == (b - k)` is `a == b`.
#[test]
fn the_same_offset_on_both_sides_drops() {
    check(
        "i16 %a, i16 %b, i16 %k",
        "  %x = sub i16 %a, %k\n  %y = sub i16 %b, %k\n  %r = icmp eq i16 %x, %y\n  ret i1 %r\n",
        &["icmp eq i16 %a, %b"],
    );
}

/// `(a << 3) == 24` for a `nuw` shift is `a == 3`.
#[test]
fn a_nuw_shift_compared_with_a_multiple_drops_the_shift() {
    check("i16 %a", "  %x = shl nuw i16 %a, 3\n  %r = icmp eq i16 %x, 24\n  ret i1 %r\n", &["icmp eq i16 %a, 3"]);
}
