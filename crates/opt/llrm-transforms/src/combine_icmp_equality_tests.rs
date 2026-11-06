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
fn combined(
    params: &str,
    body: &str,
) -> String {
    let text = format!("define i1 @f({params}) {{\nb0:\n{body}}}\n");
    let mut module = parsed(&text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// `define i1 @f(PARAMS)` with `body` before and `after` after `visitICmpInst`
/// alone.
fn check(
    params: &str,
    body: &str,
    after: &str,
) {
    let want = format!("define i1 @f({params}) {{\nb0:\n{after}}}\n");
    assert_eq!(combined(params, body), want);
}

/// `(A ^ B) == A` is `B == 0` (`opt`: `icmp eq i16 %b, 0`).
#[test]
fn an_xor_compared_with_its_operand_is_a_zero_test() {
    check(
        "i16 %a, i16 %b",
        "  %x = xor i16 %a, %b\n  %r = icmp eq i16 %x, %a\n  ret i1 %r\n",
        "  %0 = icmp eq i16 %b, 0\n  ret i1 %0\n",
    );
}

/// `(A ^ B) != (A ^ D)` is `B != D`.
#[test]
fn two_xors_with_a_shared_operand_compare_the_others() {
    check(
        "i16 %a, i16 %b, i16 %d",
        "  %x = xor i16 %a, %b\n  %y = xor i16 %a, %d\n  %r = icmp ne i16 %x, %y\n  ret i1 %r\n",
        "  %0 = icmp ne i16 %b, %d\n  ret i1 %0\n",
    );
}

/// `(A ^ 5) == (C ^ 12)` is `A == C ^ 9`; `opt` goes on to move the constant
/// (OWED).
#[test]
fn two_xors_with_constants_fold_the_constants() {
    check(
        "i16 %a, i16 %c",
        "  %x = xor i16 %a, 5\n  %y = xor i16 %c, 12\n  %r = icmp eq i16 %x, %y\n  ret i1 %r\n",
        "  %0 = xor i16 %c, 9\n  %1 = icmp eq i16 %a, %0\n  ret i1 %1\n",
    );
}

/// `(X & Z) == (Y & Z)` is `((X ^ Y) & Z) == 0`.
#[test]
fn two_ands_with_a_shared_mask_are_a_masked_zero_test() {
    check(
        "i16 %x, i16 %y, i16 %z",
        "  %p = and i16 %x, %z\n  %q = and i16 %y, %z\n  %r = icmp eq i16 %p, %q\n  ret i1 %r\n",
        "  %0 = xor i16 %x, %y\n  %1 = and i16 %0, %z\n  %2 = icmp eq i16 %1, 0\n  ret i1 %2\n",
    );
}

/// `(X | 12) != (Y | 12)` is `((X ^ Y) & ~12) != 0`.
#[test]
fn two_ors_with_a_shared_constant_are_a_masked_zero_test() {
    check(
        "i16 %x, i16 %y",
        "  %p = or i16 %x, 12\n  %q = or i16 %y, 12\n  %r = icmp ne i16 %p, %q\n  ret i1 %r\n",
        "  %0 = xor i16 %x, %y\n  %1 = and i16 %0, -13\n  %2 = icmp ne i16 %1, 0\n  ret i1 %2\n",
    );
}

/// `(B & 255) == zext A` is `A == trunc B`: strchr's compare of a byte against
/// `c`, the i16 compare the 6502 paid two bytes for (`opt`: `icmp eq i8 %y,
/// %1`).
#[test]
fn a_low_mask_against_a_zero_extension_is_a_narrow_compare() {
    check(
        "i16 %c, i8 %y",
        "  %a = and i16 %c, 255\n  %z = zext i8 %y to i16\n  %r = icmp eq i16 %a, %z\n  ret i1 %r\n",
        "  %0 = trunc i16 %c to i8\n  %1 = icmp eq i8 %y, %0\n  ret i1 %1\n",
    );
}

/// `(A >> 4) == (B >> 4)` is `(A ^ B) u< 16`.
#[test]
fn two_equal_logical_shifts_compare_by_a_range() {
    check(
        "i16 %x, i16 %y",
        "  %p = lshr i16 %x, 4\n  %q = lshr i16 %y, 4\n  %r = icmp eq i16 %p, %q\n  ret i1 %r\n",
        "  %0 = xor i16 %x, %y\n  %1 = icmp ult i16 %0, 16\n  ret i1 %1\n",
    );
}

/// `(A >>s 3) != (B >>s 3)` is `(A ^ B) u>= 8`, in the canonical `u> 7`.
#[test]
fn two_equal_arithmetic_shifts_compare_by_a_range() {
    check(
        "i16 %x, i16 %y",
        "  %p = ashr i16 %x, 3\n  %q = ashr i16 %y, 3\n  %r = icmp ne i16 %p, %q\n  ret i1 %r\n",
        "  %0 = xor i16 %x, %y\n  %1 = icmp ugt i16 %0, 7\n  ret i1 %1\n",
    );
}

/// `(A << 4) == (B << 4)` is `((A ^ B) & 4095) == 0`.
#[test]
fn two_equal_left_shifts_compare_under_a_mask() {
    check(
        "i16 %x, i16 %y",
        "  %p = shl i16 %x, 4\n  %q = shl i16 %y, 4\n  %r = icmp eq i16 %p, %q\n  ret i1 %r\n",
        "  %0 = xor i16 %x, %y\n  %1 = and i16 %0, 4095\n  %2 = icmp eq i16 %1, 0\n  ret i1 %2\n",
    );
}

/// `trunc (lshr X, 4) == 7`, with X used elsewhere, is `(X & 4080) == 112`.
#[test]
fn a_truncated_shift_with_a_shared_source_is_a_masked_compare() {
    check(
        "i16 %x",
        "  %s = lshr i16 %x, 4\n  %t = trunc i16 %s to i8\n  %r = icmp eq i8 %t, 7\n  %w = icmp ult i16 %x, 1000\n  %v = xor i1 %r, %w\n  ret i1 %v\n",
        "  %0 = and i16 %x, 4080\n  %1 = icmp eq i16 %0, 112\n  %w = icmp ult i16 %x, 1000\n  %v = xor i1 %1, %w\n  ret i1 %v\n",
    );
}

/// The top byte-and-one bits all alike: `A >=s -128 && A <=s 127` as `(A + 128)
/// u< 256`.
#[test]
fn a_sign_extension_check_of_a_trunc_is_a_range_test() {
    check(
        "i16 %a",
        "  %t = trunc i16 %a to i8\n  %p = ashr i8 %t, 7\n  %s = lshr i16 %a, 8\n  %q = trunc i16 %s to i8\n  %r = icmp eq i8 %p, %q\n  ret i1 %r\n",
        "  %0 = add i16 %a, 128\n  %1 = icmp ult i16 %0, 256\n  ret i1 %1\n",
    );
}
