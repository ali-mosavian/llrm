//! The compare steps of CodeGenPrepare's `optimizeCmp` after the overflow
//! combines.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::lowering::Lowering;

use super::compares_optimized;
use crate::testing::{parsed, printed};

struct Plain;
impl Lowering for Plain {}

/// A target whose equality compare is not lowered with the signed one beside
/// it (PowerPC's answer).
struct SignedFree;
impl Lowering for SignedFree {
    fn is_equality_cmp_folded_with_signed_cmp(&self) -> bool {
        false
    }
}

fn run(
    lowering: &dyn Lowering,
    text: &str,
) -> (bool, String) {
    let mut module = parsed(&format!("declare i16 @llvm.ctpop.i16(i16)\n\n{text}"));
    let layout = DataLayout::default();
    let globals = module.globals.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = compares_optimized(context, &globals, &layout, lowering, function);
    (changed, printed(&module))
}

/// A power-of-two test of a count known non-zero (`x | 1`) was a `ctpop`, a
/// call without POPCNT; it is `(x & (x - 1)) == 0`.
#[test]
fn test_ctpop_equal_one_of_a_nonzero_value_is_an_and_with_its_predecessor() {
    let (changed, after) = run(
        &Plain,
        "define i1 @f(i16 %x) {
b0:
  %n = or i16 %x, 1
  %p = call i16 @llvm.ctpop.i16(i16 %n)
  %c = icmp eq i16 %p, 1
  ret i1 %c
}
",
    );
    assert!(changed && !after.contains("call i16 @llvm.ctpop"), "{after}");
    assert!(after.contains("and i16 %n,") && after.contains("icmp eq i16"), "{after}");
}

/// `ctpop(x) u< 2` holds for zero too: `(x & (x - 1)) == 0` without the
/// non-zero proof; `ctpop(x) == 1` of an unknown x is the xor form.
#[test]
fn test_ctpop_below_two_is_an_and_and_equal_one_of_an_unknown_value_is_an_xor() {
    let (_, below) = run(
        &Plain,
        "define i1 @f(i16 %x) {
b0:
  %p = call i16 @llvm.ctpop.i16(i16 %x)
  %c = icmp ult i16 %p, 2
  ret i1 %c
}
",
    );
    assert!(below.contains("and i16 %x,") && below.contains("icmp eq i16"), "{below}");
    let (_, equal) = run(
        &Plain,
        "define i1 @f(i16 %x) {
b0:
  %p = call i16 @llvm.ctpop.i16(i16 %x)
  %c = icmp eq i16 %p, 1
  ret i1 %c
}
",
    );
    assert!(equal.contains("xor i16 %x,") && equal.contains("icmp ugt i16"), "{equal}");
}

/// Another bound is no power-of-two test.
#[test]
fn test_ctpop_compared_with_three_stays() {
    let (changed, after) = run(
        &Plain,
        "define i1 @f(i16 %x) {
b0:
  %p = call i16 @llvm.ctpop.i16(i16 %x)
  %c = icmp eq i16 %p, 3
  ret i1 %c
}
",
    );
    assert!(!changed && after.contains("call i16 @llvm.ctpop"), "{after}");
}

/// A compare of `a` and `b` beside `sub b, a` reads `b`, `a`: one instruction
/// on a target that subtracts to compare. With `sub a, b` beside it, it stays.
#[test]
fn test_a_compare_swaps_to_match_a_reversed_subtract() {
    let text = |sub: &str| {
        format!(
            "define i1 @f(i16 %a, i16 %b) {{
b0:
  %d = sub i16 {sub}
  %c = icmp slt i16 %a, %b
  %z = icmp eq i16 %d, 0
  %r = and i1 %c, %z
  ret i1 %r
}}
"
        )
    };
    let (changed, after) = run(&Plain, &text("%b, %a"));
    assert!(changed && after.contains("icmp sgt i16 %b, %a"), "{after}");
    let (changed, after) = run(&Plain, &text("%a, %b"));
    assert!(!changed && after.contains("icmp slt i16 %a, %b"), "{after}");
}

/// `a == b` where `a < b` was false is `a > b` with the arms swapped, on a
/// target that does not lower an equality beside the signed compare; x86
/// (the default answer) leaves it.
#[test]
fn test_an_equality_after_a_signed_compare_is_the_opposite_compare_where_the_target_asks() {
    let text = "define i16 @f(i16 %a, i16 %b) {
b0:
  %lt = icmp slt i16 %a, %b
  br i1 %lt, label %less, label %b1

b1:
  %eq = icmp eq i16 %a, %b
  br i1 %eq, label %same, label %more

less:
  ret i16 1

same:
  ret i16 2

more:
  ret i16 3
}
";
    let (changed, after) = run(&Plain, text);
    assert!(!changed && after.contains("icmp eq i16 %a, %b"), "{after}");
    let (changed, after) = run(&SignedFree, text);
    assert!(changed && after.contains("icmp sgt i16 %a, %b"), "{after}");
    assert!(after.contains("br i1 %eq, label %more, label %same"), "{after}");
}
