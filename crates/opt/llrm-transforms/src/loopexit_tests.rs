//! Adapted from llrm-core's `optimize/loopexit_tests.rs`, the port of
//! `tests/test_loopexit.py`, each body now MIR text run by llrm-mir's
//! interpreter before and after.
//!
//! The hand-built `i < 4; s += i` loop keeps its three tests. Skipped, BC
//! and OMF fixtures through the old pipeline:
//! `test_addrm_long_sum_is_computed_outside_the_store_loop` and
//! `test_accumulation_has_no_backedge`.

use llrm_analysis::cfg;
use llrm_analysis::testing::{DOS, layout};
use llrm_analysis::graph::loops;
use llrm_mir::module::Module;
use llrm_mir::passes::Outer;

use super::evaluated;
use crate::testing::{parsed, printed, results};

/// @f evaluated; whether anything changed.
fn evaluate(module: &mut Module) -> bool {
    let (layout, outer) = (layout(module), Outer::of(module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    evaluated(context, &layout, function, &outer).unwrap()
}

/// `text` evaluated: its printed form, whether it changed, and how many
/// loops it keeps. @f computes what it did on `inputs`.
fn evaluated_text(text: &str, inputs: &[&[i128]]) -> (String, bool, usize) {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let changed = evaluate(&mut module);
    let after = printed(&module);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).unwrap();
    (after, changed, loops::loops(&cfg::graph(function), None).len())
}

/// `i` from 0 while `i < 4`, `s += i`, and the latch's `stored` line.
fn counted(stored: &str) -> String {
    format!(
        "@g = global i16 0

define i16 @f() {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %s = phi i16 [ 0, %b0 ], [ %s1, %b2 ]
  %c = icmp uge i16 %i, 4
  br i1 %c, label %b3, label %b2

b2:
  %s1 = add i16 %s, %i
  %next = add i16 %i, 1
{stored}  br label %b1

b3:
  ret i16 %s
}}
"
    )
}

#[test]
fn disposable_loop_becomes_its_exit_values() {
    let (text, changed, kept) = evaluated_text(&counted(""), &[&[]]);
    assert!(changed && kept == 0);
    assert!(text.ends_with("define i16 @f() {\nb0:\n  br label %b1\n\nb1:\n  br label %b3\n\nb3:\n  ret i16 6\n}\n"), "{text}");
}

/// An accumulator a store reads stays in the loop, and the read after the
/// loop takes its constant exit value. That read kept `%s` live after the
/// loop, and with it whatever counter computed it.
#[test]
fn accumulator_read_by_a_store_is_kept_and_its_exit_is_constant() {
    let text = counted("  store i16 %s1, ptr @g\n");
    let (after, changed, kept) = evaluated_text(&text, &[&[]]);
    assert!(changed && kept == 1, "{after}");
    assert!(after.contains("store i16 %s1, ptr @g") && after.contains("ret i16 6"), "{after}");
}

#[test]
fn constant_exit_replaces_an_unobserved_accumulator() {
    let (text, changed, kept) = evaluated_text(&counted("  store i16 %next, ptr @g\n"), &[&[]]);
    assert!(changed && kept == 1);
    assert!(text.ends_with("b3:\n  ret i16 6\n}\n"), "{text}");
}

/// A volatile access is observable work: the loop stays, and only the sum
/// it no longer needs to compute leaves it.
#[test]
fn a_loop_with_a_volatile_access_stays() {
    let (text, changed, kept) = evaluated_text(&counted("  %v = load volatile i16, ptr @g\n"), &[&[]]);
    assert!(changed && kept == 1);
    assert!(text.contains("load volatile") && text.ends_with("b3:\n  ret i16 6\n}\n"), "{text}");
}

/// `trips` trips from %a: `i`, `s += i` from %k and `t -= k` from 0, all
/// read after the loop.
fn symbolic(trips: u32) -> String {
    format!(
        "define i16 @f(i16 %a, i16 %k) {{
b0:
  %end = add i16 %a, {trips}
  br label %b1

b1:
  %i = phi i16 [ %a, %b0 ], [ %next, %b2 ]
  %s = phi i16 [ %k, %b0 ], [ %s1, %b2 ]
  %t = phi i16 [ 0, %b0 ], [ %t1, %b2 ]
  %c = icmp eq i16 %i, %end
  br i1 %c, label %b3, label %b2

b2:
  %s1 = add i16 %s, %i
  %t1 = sub i16 %t, %k
  %next = add i16 %i, 1
  br label %b1

b3:
  %u = xor i16 %s, %t
  %r = add i16 %u, %i
  ret i16 %r
}}
"
    )
}

const PAIRS: &[&[i128]] = &[&[0, 0], &[5, 3], &[-7, 100], &[32760, -2]];

#[test]
fn symbolic_exit_values_replace_the_loop() {
    for trips in [1, 2, 100] {
        let (text, changed, kept) = evaluated_text(&symbolic(trips), PAIRS);
        assert!(changed && kept == 0, "{text}");
        assert!(!text.contains("phi"), "{text}");
    }
    let (text, _, _) = evaluated_text(&symbolic(100), PAIRS);
    assert!(
        text.contains("b1:\n  %0 = add i16 %a, 100\n  %1 = mul i16 %a, 100\n  %2 = add i16 %k, %1\n  %3 = add i16 %2, 4950\n  %4 = mul i16 %k, -100\n  %5 = add i16 0, %4\n  br label %b3\n"),
        "{text}"
    );
}

/// No trip is no agreed count: the loop stays.
#[test]
fn a_loop_of_no_trips_is_kept() {
    let (text, changed, kept) = evaluated_text(&symbolic(0), PAIRS);
    assert!(!changed && kept == 1, "{text}");
}

/// `s += s` is no linear sum: its exit value is not computable, and the
/// loop computing it stays; the counter's, 4, replaces its read.
#[test]
fn an_exit_value_used_but_not_computable_keeps_the_loop() {
    let text = counted("").replace("%s1 = add i16 %s, %i", "%s1 = add i16 %s, %s").replace("ret i16 %s", "%r = add i16 %s, %i\n  ret i16 %r");
    let (after, changed, kept) = evaluated_text(&text, &[&[]]);
    assert!(changed && kept == 1, "{after}");
    assert!(after.contains("%r = add i16 %s, 4"), "{after}");
}

/// A narrow counter widened by `sext` sums as the wide recurrence ranges
/// bound it to.
#[test]
fn a_widened_counter_sums_at_its_wide_width() {
    let text = "define i32 @f() {
b0:
  br label %b1

b1:
  %i = phi i16 [ -3, %b0 ], [ %next, %b2 ]
  %s = phi i32 [ 100000, %b0 ], [ %s1, %b2 ]
  %c = icmp slt i16 %i, 20
  br i1 %c, label %b2, label %b3

b2:
  %w = sext i16 %i to i32
  %s1 = add i32 %s, %w
  %next = add i16 %i, 1
  br label %b1

b3:
  ret i32 %s
}
";
    let (after, changed, kept) = evaluated_text(text, &[&[]]);
    assert!(changed && kept == 1);
    assert!(after.ends_with("b3:\n  ret i32 100184\n}\n"), "{after}");
}

/// suite/hotlpx summed `n * k` over twenty trips, an invariant whatever
/// computes it, and suite/arridx summed `a(i) + a(i)`, `i * 3` twice, a
/// multiple of the counter. Both sums were evaluated only once strength
/// reduction had made each a phi of its own.
#[test]
fn a_sum_of_an_invariant_product_and_a_counter_multiple_is_evaluated() {
    for update in ["%q = add i16 %p, 0", "%three = mul i16 %i, 3\n  %q = add i16 %three, %three"] {
        let text = format!(
            "define i16 @f(i16 %a, i16 %k) {{
b0:
  %p = mul i16 %a, %k
  br label %b1

b1:
  %i = phi i16 [ 1, %b0 ], [ %next, %b2 ]
  %s = phi i16 [ 0, %b0 ], [ %s1, %b2 ]
  %c = icmp sgt i16 %i, 20
  br i1 %c, label %b3, label %b2

b2:
  {update}
  %s1 = add i16 %s, %q
  %next = add i16 %i, 1
  br label %b1

b3:
  ret i16 %s
}}
"
        );
        let (after, changed, _) = evaluated_text(&text, PAIRS);
        assert!(changed && !after.contains("ret i16 %s"), "{after}");
    }
}
