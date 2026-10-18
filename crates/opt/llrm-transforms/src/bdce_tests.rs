//! Each body is MIR text run by llrm-mir's interpreter before and after; the
//! expected text is what `opt -passes=bdce` makes of the same body (checked
//! with `tools/difftest`).

use llrm_mir::module::Module;
use llrm_mir::passes::Outer;

use super::bit_tracking_dce;
use crate::testing::{parsed, printed, results};

/// @f after BDCE; whether anything changed.
fn bdced(module: &mut Module) -> bool {
    let outer = Outer::of(module, None);
    let layout = llrm_analysis::testing::layout(module);
    let (context, function) = module.function_mut("f").expect("@f");
    bit_tracking_dce(context, &layout, &outer, function)
}

/// `text` after BDCE is `expected` and computes what it did on `inputs`.
fn check(
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
    let mut module = parsed(text);
    let before = results(&module, inputs);
    assert_eq!(bdced(&mut module), text != expected);
    assert_eq!(printed(&module), expected);
    assert_eq!(results(&module, inputs), before);
}

/// A sign extension whose extension bits an `and 255` never reads is a zero
/// extension: `movsx` became `movzx`.
#[test]
fn test_a_sign_extension_nobody_reads_the_top_of_is_a_zero_extension() {
    check(
        "define i32 @f(i8 %x) {
b0:
  %s = sext i8 %x to i32
  %m = and i32 %s, 255
  ret i32 %m
}
",
        "define i32 @f(i8 %x) {
b0:
  %s1 = zext i8 %x to i32
  %m = and i32 %s1, 255
  ret i32 %m
}
",
        &[&[0], &[-1], &[127], &[-128]],
    );
}

/// The new instruction takes the old name, uniqued as LLVM's symbol table does
/// (`%s1`), so the diff against `opt` has no rename.

/// A mask that clears only bits the store of a byte never reads is its
/// operand.
#[test]
fn test_a_mask_over_unread_bits_is_its_operand() {
    check(
        "define i8 @f(i32 %x) {
b0:
  %a = add i32 %x, 1
  %m = and i32 %a, 255
  %t = trunc i32 %m to i8
  ret i8 %t
}
",
        "define i8 @f(i32 %x) {
b0:
  %a = add i32 %x, 1
  %t = trunc i32 %a to i8
  ret i8 %t
}
",
        &[&[0], &[255], &[-1], &[1000]],
    );
}

/// `and %v, 0` reads none of `%v`: the computation of `%v` goes and the
/// operand is zero.
#[test]
fn test_an_operand_whose_bits_are_all_unread_is_zero_and_its_computation_goes() {
    check(
        "define i32 @f(i32 %x) {
b0:
  %v = mul i32 %x, 3
  %m = and i32 %v, 0
  ret i32 %m
}
",
        "define i32 @f(i32 %x) {
b0:
  %m = and i32 0, 0
  ret i32 %m
}
",
        &[&[0], &[5]],
    );
}

/// A sign extension turned zero extension changes the operand of an `add nsw`
/// downstream: the flag was proved of the old value and goes.
#[test]
fn test_a_rewrite_drops_the_flags_proved_of_the_old_operand() {
    check(
        "define i8 @f(i8 %x) {
b0:
  %s = sext i8 %x to i32
  %a = add nsw i32 %s, 1
  %t = trunc i32 %a to i8
  ret i8 %t
}
",
        "define i8 @f(i8 %x) {
b0:
  %s1 = zext i8 %x to i32
  %a = add i32 %s1, 1
  %t = trunc i32 %a to i8
  ret i8 %t
}
",
        &[&[0], &[127], &[-128], &[-1]],
    );
}

/// Nothing to do changes nothing.
#[test]
fn test_a_body_that_reads_every_bit_is_left_alone() {
    let text = "define i32 @f(i32 %x) {
b0:
  %a = add i32 %x, 1
  ret i32 %a
}
";
    check(text, text, &[&[0], &[-1]]);
}
