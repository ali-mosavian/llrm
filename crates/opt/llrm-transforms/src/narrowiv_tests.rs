//! A counter whose range fits a native width is that width.
//!
//! Each test runs @f through `indvars` on an 8/16-bit datalayout and compares
//! what the interpreter computes before and after.

use crate::indvars::IndVars;
use crate::testing::{Tuned, managed_on, parsed, printed, results};

const Z80: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-n8:16\"\n\n";

/// `for (i = start; test(i + step); i += step) sum = (sum + i) * (n | 3)` over
/// `ty`, then `sum + n`: no closed form, so the loop stays.
fn counting(
    ty: &str,
    start: i64,
    op: &str,
    step: i64,
    test: &str,
) -> String {
    let widen = if ty == "i32" { "or i32 %i, 0".to_owned() } else { format!("zext {ty} %i to i32") };
    format!(
        "{Z80}define i32 @f(i32 %n) {{
entry:
  %m = or i32 %n, 3
  br label %loop
loop:
  %i = phi {ty} [ {start}, %entry ], [ %next, %loop ]
  %s = phi i32 [ 0, %entry ], [ %acc, %loop ]
  %w = {widen}
  %t = add i32 %s, %w
  %acc = mul i32 %t, %m
  %next = {op} {ty} %i, {step}
  %c = {test}
  br i1 %c, label %loop, label %exit
exit:
  %out = phi i32 [ %acc, %loop ]
  %r = add i32 %out, %n
  ret i32 %r
}}
"
    )
}

/// @f of `text` through `indvars` on a target that narrows counters or not.
fn run(
    text: &str,
    narrows_counters: bool,
) -> String {
    let mut module = parsed(text);
    let inputs: &[&[i128]] = &[&[0], &[5], &[1000]];
    let before = results(&module, inputs);
    managed_on(&mut module, IndVars { lftr: true, size: false }, Tuned { narrows_counters, ..Default::default() });
    let after = printed(&module);
    assert_eq!(results(&module, inputs), before, "{after}");
    after
}

/// Symptom: `for (long i = 0; i < 12; ++i)` compared and stepped four bytes on
/// an 8-bit target (div_constants_short -Oz: 14 B a compare, 1189 B a program).
#[test]
fn a_twelve_trip_counter_of_four_bytes_is_one_byte() {
    let after = run(&counting("i32", 0, "add", 1, "icmp ult i32 %next, 12"), true);
    assert!(after.contains("phi i8 [ 0, %entry ]"), "{after}");
    assert!(after.contains("add i8"), "{after}");
    assert!(!after.contains("phi i32 [ 0, %entry ], [ %next"), "{after}");
}

#[test]
fn a_counter_of_a_thousand_trips_is_two_bytes() {
    let after = run(&counting("i32", 0, "add", 1, "icmp ult i32 %next, 1000"), true);
    assert!(after.contains("phi i16 [ 0, %entry ]"), "{after}");
}

/// The last value is one past the last trip: 255 trips end at 255, 256 trips at
/// 256, which does not fit a byte.
#[test]
fn the_value_after_the_last_trip_counts_toward_the_width() {
    let fits = run(&counting("i32", 0, "add", 1, "icmp ult i32 %next, 255"), true);
    assert!(fits.contains("phi i8 [ 0, %entry ]"), "{fits}");
    let over = run(&counting("i32", 0, "add", 1, "icmp ult i32 %next, 256"), true);
    assert!(over.contains("phi i16 [ 0, %entry ]"), "{over}");
}

/// A countdown ends at -1: no unsigned width holds it, a signed byte does.
#[test]
fn a_countdown_that_ends_below_zero_is_a_signed_byte() {
    let after = run(&counting("i32", 11, "sub", 1, "icmp sgt i32 %next, -1"), true);
    assert!(after.contains("phi i8 [ 11, %entry ]"), "{after}");
    assert!(after.contains("sext i8"), "{after}");
}

/// No bound on the trips, no bound on the counter.
#[test]
fn a_counter_with_an_unknown_trip_count_stays() {
    let after = run(&counting("i32", 0, "add", 1, "icmp ult i32 %next, %n"), true);
    assert!(!after.contains("phi i8") && !after.contains("phi i16"), "{after}");
}

#[test]
fn a_target_that_does_not_ask_keeps_the_four_byte_counter() {
    let after = run(&counting("i32", 0, "add", 1, "icmp ult i32 %next, 12"), false);
    assert!(!after.contains("phi i8") && !after.contains("phi i16"), "{after}");
}

/// A counter of a native width is the target's to keep.
#[test]
fn a_counter_of_a_native_width_stays() {
    let after = run(&counting("i16", 0, "add", 1, "icmp ult i16 %next, 12"), true);
    assert!(after.contains("phi i16 [ 0, %entry ]") && !after.contains("phi i8"), "{after}");
}

/// A step that carries the counter past 255 between tests: 0, 100, 200, 300
/// does not wrap a byte, and must not become one.
#[test]
fn a_counter_stepping_over_the_byte_edge_is_not_a_byte() {
    let after = run(&counting("i32", 0, "add", 100, "icmp ult i32 %next, 300"), true);
    assert!(after.contains("phi i16 [ 0, %entry ]") && !after.contains("phi i8"), "{after}");
}
