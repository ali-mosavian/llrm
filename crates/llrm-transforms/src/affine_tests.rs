//! `Affine` over loops the interpreter runs before and after, over trip
//! counts 0, 1 and more. The old module had no tests.

use llrm_analysis::testing::DOS;

use super::Affine;
use crate::testing::{managed, parsed, printed, results};

/// `for (i = 0; i < n; i++) s += BODY`, the loop's value `%v`.
fn looped(body: &str) -> String {
    format!(
        "define void @g() {{
b0:
  ret void
}}

define i16 @f(i16 %n, i16 %k) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
  %s = phi i16 [ 0, %b0 ], [ %s.next, %b2 ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %b2, label %b3

b2:
{body}  %s.next = add i16 %s, %v
  %i.next = add i16 %i, 1
  br label %b1

b3:
  ret i16 %s
}}
"
    )
}

const TRIPS: &[&[i128]] = &[&[0, 3], &[1, 3], &[2, -9], &[11, 700]];

/// `text` through `Affine`, printed, where it computes what it did.
fn spelled(text: &str) -> (String, String) {
    let before = parsed(&format!("{DOS}{text}"));
    let mut after = before.clone();
    let printed_after = managed(&mut after, Affine);
    assert_eq!(results(&after, TRIPS), results(&before, TRIPS), "{printed_after}");
    (printed(&before), printed_after)
}

#[test]
fn test_a_nested_affine_value_is_the_scaled_counter_plus_a_base() {
    let (_, after) = spelled(&looped("  %a = sub i16 %i, %k\n  %b = add i16 %a, 640\n  %v = mul i16 %b, 2\n"));
    assert!(after.contains("  %0 = mul i16 %i, 2\n  %1 = mul i16 %k, -2\n  %2 = mul i16 640, 2\n  %3 = add i16 %1, %2\n  %4 = add i16 %0, %3\n"), "{after}");
    assert!(after.contains("  %s.next = add i16 %s, %4\n"), "{after}");
    let mut again = parsed(&after);
    assert_eq!(managed(&mut again, Affine), after);
}

#[test]
fn test_an_affine_value_with_no_invariant_value_is_left() {
    let text = looped("  %a = add i16 %i, 5\n  %v = mul i16 %a, 3\n");
    let (before, after) = spelled(&text);
    assert_eq!(after, before);
}

#[test]
fn test_a_value_already_scaled_plus_base_is_left() {
    let (before, after) = spelled(&looped("  %m = mul i16 %i, 2\n  %v = add i16 %m, %k\n"));
    assert_eq!(after, before);
}

/// The old hoist refused a loop holding a call, so a base there stays put.
#[test]
fn test_a_loop_holding_a_call_is_left() {
    let (before, after) = spelled(&looped("  call void @g()\n  %a = sub i16 %i, %k\n  %v = mul i16 %a, 2\n"));
    assert_eq!(after, before);
}
