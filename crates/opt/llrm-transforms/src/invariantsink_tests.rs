//! `sunk` over nested loops the interpreter runs before and after.

use llrm_mir::memory::Callees;
use llrm_mir::module::Function;

use super::sunk as sunk_with;
use crate::testing::{f, parsed, printed, results};

/// An outer loop whose counter's increment sits in the inner loop's
/// preheader, `held` beside it.
pub(crate) fn nested(held: &str) -> String {
    format!(
        "define i32 @f(i32 %n, i32 %m) {{
b0:
  br label %outer

outer:
  %i = phi i32 [ 0, %b0 ], [ %i.next, %latch ]
  %acc = phi i32 [ 0, %b0 ], [ %acc2, %latch ]
  %i.next = add i32 %i, 1
{held}  br label %inner

inner:
  %j = phi i32 [ 0, %outer ], [ %j.next, %inner ]
  %a = phi i32 [ %acc, %outer ], [ %a2, %inner ]
  %a2 = add i32 %a, %j
  %j.next = add i32 %j, 1
  %jc = icmp ult i32 %j.next, %m
  br i1 %jc, label %inner, label %latch

latch:
  %acc2 = phi i32 [ %a2, %inner ]
  %go = icmp ult i32 %i.next, %n
  br i1 %go, label %outer, label %exit

exit:
  %r = phi i32 [ %acc2, %latch ]
  ret i32 %r
}}
"
    )
}

/// `function` sunk, its loops found for it.
fn sunk(module: &mut llrm_mir::module::Module) -> bool {
    let context = module.context.clone();
    let globals = module.globals.clone();
    let function: &mut Function = f(module);
    let loops = llrm_analysis::cfg::loops_of(function);
    sunk_with(&context, &Callees::default(), &globals, function, &loops)
}

/// `text` sunk, printed, where it computes what it did.
fn run(text: &str) -> (String, bool) {
    let before = parsed(text);
    let mut after = before.clone();
    let changed = sunk(&mut after);
    let text = printed(&after);
    let inputs: &[&[i128]] = &[&[1, 1], &[3, 2], &[4, 0], &[2, 5]];
    assert_eq!(results(&after, inputs), results(&before, inputs), "{text}");
    (text, changed)
}

/// grep -Os (+2 B at wasm32): the outer counter's `add` stayed in the inner
/// loop's preheader, where the pin moves it to the latch that reads it, so
/// the value before the increment lived across the inner loop in a second
/// local.
#[test]
fn test_an_increment_only_the_latch_reads_moves_there() {
    let (after, changed) = run(&nested(""));
    assert!(changed);
    assert!(after.contains("latch:\n  %acc2 = phi i32 [ %a2, %inner ]\n  %i.next = add i32 %i, 1\n"), "{after}");
    assert!(!after.contains("outer:\n  %i = phi i32 [ 0, %b0 ], [ %i.next, %latch ]\n  %acc = phi i32 [ 0, %b0 ], [ %acc2, %latch ]\n  %i.next"), "{after}");
}

/// A value the loop reads stays: `%s` feeds the inner sum.
#[test]
fn test_a_value_the_loop_reads_stays() {
    let text =
        nested("  %s = add i32 %i, 7\n").replace("%a2 = add i32 %a, %j", "%t = add i32 %j, %s\n  %a2 = add i32 %a, %t");
    let (after, _) = run(&text);
    assert!(after.contains("outer:\n  %i = phi i32 [ 0, %b0 ], [ %i.next, %latch ]\n  %acc = phi i32 [ 0, %b0 ], [ %acc2, %latch ]\n  %s = add i32 %i, 7\n"), "{after}");
}

/// The walk is backwards, so a value only a moved value reads goes with it.
#[test]
fn test_a_chain_moves_together() {
    let text = nested("  %u = add i32 %i, 3\n  %v = mul i32 %u, 5\n")
        .replace("%go = icmp ult i32 %i.next, %n", "%w = add i32 %v, %i.next\n  %go = icmp ult i32 %w, %n");
    let (after, changed) = run(&text);
    assert!(changed);
    let (outer, _) = after.split_once("inner:").expect("inner");
    assert!(!outer.contains("%u = ") && !outer.contains("%v = "), "{after}");
}
