//! `narrow_induction_chains`: what the loop computes from its counter is done
//! in the narrowest lane that holds it.

use llrm_mir::module::{BlockId, Operand};

use super::narrow_induction_chains;
use crate::testing::{f, parsed, printed, results};

const LAYOUT: &str =
    "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"";

/// `sum += ((i & 15) + 1) * i` over an `i64` counter, with a 16-bit load beside
/// it (the loop's narrowest access).
fn weighted(access: &str) -> String {
    format!(
        "{LAYOUT}

define i64 @f(i64 %n) {{
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %s = phi i64 [ 0, %entry ], [ %sum, %loop ]
  %m = and i64 %i, 15
  %w = add nuw nsw i64 %m, 1
  %t = mul i64 %w, %i
  %sum = add i64 %s, %t
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, %n
  br i1 %done, label %exit, label %loop
exit:
  %r = phi i64 [ %sum, %loop ]
  ret i64 %r
}}
{access}"
    )
}

fn narrowed(text: &str) -> (String, bool) {
    let mut module = parsed(text);
    let changed = {
        let (context, function) = module.function_mut("f").expect("@f");
        let phi =
            function.instruction(function.iter_block(BlockId(1)).next().expect("a phi")).result.expect("a result");
        narrow_induction_chains(context, function, BlockId(1), Operand::Value(phi))
    };
    (printed(&module), changed)
}

/// Before: the weight was an `i64` chain (`and i64`, `add i64`), 8 lanes of 64
/// bits a vector of four registers, then shuffled down to the 16-bit multiply:
/// scroll +3.8k instructions against gcc.
#[test]
fn a_masked_counter_chain_is_made_in_sixteen_bit_lanes() {
    let (text, changed) = narrowed(&weighted(""));
    assert!(changed, "{text}");
    assert!(text.contains("trunc i64 %i to i16"), "{text}");
    assert!(text.contains("and i16") && text.contains("add i16"), "{text}");
    assert!(!text.contains("and i64") && !text.contains("%w = ") && !text.contains("%m = "), "{text}");
    // 5 bits in a 16-bit lane: the sign bit is clear, so the extension is
    // `sext`, the multiply the target prices.
    assert!(text.contains("sext i16"), "{text}");
}

/// The narrowed loop returns what the wide one did, for every trip count.
#[test]
fn a_narrowed_chain_computes_what_the_wide_one_did() {
    let inputs: Vec<Vec<i128>> = (1..70).map(|n| vec![n]).collect();
    let inputs: Vec<&[i128]> = inputs.iter().map(Vec::as_slice).collect();
    let before = results(&parsed(&weighted("")), &inputs);
    let mut module = parsed(&weighted(""));
    {
        let (context, function) = module.function_mut("f").expect("@f");
        let phi =
            function.instruction(function.iter_block(BlockId(1)).next().expect("a phi")).result.expect("a result");
        assert!(narrow_induction_chains(context, function, BlockId(1), Operand::Value(phi)));
    }
    assert_eq!(results(&module, &inputs), before);
}

/// A loop whose narrowest access is 64 bits has no narrower lane to put the
/// chain in: left as it is.
#[test]
fn a_chain_is_not_narrowed_below_the_loops_narrowest_access() {
    let text = weighted("").replace(
        "  %w = add nuw nsw i64 %m, 1",
        "  %q = getelementptr i64, ptr @g, i64 %i\n  %l = load i64, ptr %q\n  %w = add nuw nsw i64 %m, 1",
    ) + "@g = global [64 x i64] zeroinitializer\n";
    let (after, changed) = narrowed(&text);
    assert!(!changed, "{after}");
}

/// A result that fills the lane keeps `zext`: 16 bits of `(i & 0xffff)` have
/// the sign bit set.
#[test]
fn a_chain_that_fills_its_lane_is_zero_extended() {
    let text = weighted("").replace("and i64 %i, 15", "and i64 %i, 65535");
    let (after, changed) = narrowed(&text);
    assert!(changed, "{after}");
    assert!(after.contains("zext i16") || after.contains("zext i32"), "{after}");
}

/// A value the loop loads is not made of the counter: its chain stays wide.
#[test]
fn a_chain_of_a_loaded_value_is_left_alone() {
    let _ = f;
    let text = weighted("").replace("  %m = and i64 %i, 15", "  %m = and i64 %s, 15");
    let (after, changed) = narrowed(&text);
    assert!(!changed, "{after}");
}

/// `s += (i * 3) ^ (i >> 2)` counting down over `i32`: every result needs the
/// whole lane, so nothing narrows.
#[test]
fn a_chain_that_needs_the_whole_lane_is_left_alone() {
    let text = format!(
        "{LAYOUT}

define i32 @f(i32 %n) {{
entry:
  br label %loop
loop:
  %i = phi i32 [ %n, %entry ], [ %next, %loop ]
  %s = phi i32 [ 0, %entry ], [ %sum, %loop ]
  %a = mul i32 %i, 3
  %b = lshr i32 %i, 2
  %c = xor i32 %a, %b
  %sum = add i32 %c, %s
  %next = add i32 %i, -1
  %done = icmp eq i32 %i, 0
  br i1 %done, label %exit, label %loop
exit:
  ret i32 %sum
}}
"
    );
    let (after, changed) = narrowed(&text);
    assert!(!changed, "{after}");
}
