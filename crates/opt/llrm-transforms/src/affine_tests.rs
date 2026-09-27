//! `Affine` over loops the interpreter runs before and after, over trip
//! counts 0, 1 and more. The old module had no tests.

use llrm_analysis::testing::DOS;

use super::Affine;
use crate::testing::{managed, parsed, printed, results};

/// `for (i = 0; i < n; i++) s += BODY`, the loop's value `%v`.
fn looped(body: &str) -> String {
    format!(
        "@w = global i16 0
@a = global [300 x i16] zeroinitializer

define void @g() {{
b0:
  store i16 1, ptr @w
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

/// A call, even one that writes memory, disturbs no base: a base is
/// arithmetic on values the loop does not define, which hoisting moves
/// across any call.
#[test]
fn test_a_call_that_writes_memory_leaves_the_base_spelled() {
    let (_, after) = spelled(&looped("  call void @g()\n  %a = sub i16 %i, %k\n  %v = mul i16 %a, 2\n"));
    assert!(after.contains("  %0 = mul i16 %i, 2\n  %1 = mul i16 %k, -2\n  %2 = add i16 %0, %1\n"), "{after}");
}

/// deedlines' CYCLEBLOBS stepped seven pointers `f(x - xp(k))`, six of them
/// in memory: an address was never spelled as its invariant base indexed by
/// the scaled counter, so the terms shared no `x * 2`.
#[test]
fn test_an_affine_address_is_its_base_indexed_by_the_scaled_counter() {
    let text = looped("  %x = add i16 %i, %m\n  %p = getelementptr inbounds i16, ptr @a, i16 %x\n  %v = load i16, ptr %p\n");
    let (_, after) = spelled(&text.replace("b0:\n  br label %b1", "b0:\n  %m = and i16 %k, 255\n  br label %b1"));
    assert!(after.contains("  %0 = mul i16 %i, 2\n"), "{after}");
    assert!(after.contains(", i16 %0\n  %v = load i16, ptr %"), "{after}");
    let mut again = parsed(&after);
    assert_eq!(managed(&mut again, Affine), after);
}

/// Nib's matmul_fixed read `a[i * 8 + j]` and `b[i * 8 + j]` through one
/// index; spelled off a base each, they held two bases in memory.
#[test]
fn test_addresses_sharing_one_index_are_left() {
    let text = looped("  %x = add i16 %i, %m\n  %p = getelementptr inbounds i16, ptr @a, i16 %x\n  %q = getelementptr inbounds i8, ptr @a, i16 %x\n  %y = load i16, ptr %p\n  %z = load i8, ptr %q\n  %e = zext i8 %z to i16\n  %v = add i16 %y, %e\n");
    let (before, after) = spelled(&text.replace("b0:\n  br label %b1", "b0:\n  %m = and i16 %k, 255\n  br label %b1"));
    assert_eq!(after, before);
}
