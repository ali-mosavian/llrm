//! The remainder of a loop's counter by an invariant is a counter that wraps.

use crate::cgpaddr::CgpAddr;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\n\n";

fn after(text: &str) -> String {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(CgpAddr);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    printed(&module)
}

fn counter(body: &str) -> String {
    format!(
        "define void @f(i32 %n, i32 %m, ptr %p) {{
entry:
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
{body}
  %next = add nuw i32 %i, 1
  %c = icmp ult i32 %next, %n
  br i1 %c, label %loop, label %exit

exit:
  ret void
}}
"
    )
}

/// `i % m` in a loop that counts from zero by one divided every iteration; a
/// phi that counts and drops to zero at `m` does not.
#[test]
fn test_a_remainder_of_a_counter_by_an_invariant_is_a_wrapping_counter() {
    let text = after(&counter("  %r = urem i32 %i, %m\n  store i32 %r, ptr %p"));
    assert!(!text.contains("urem"), "{text}");
    assert!(text.contains("icmp eq i32"), "{text}");
    assert!(text.contains("select i1"), "{text}");
}

/// A constant remainder amount is a multiply, not worth a second counter; a
/// counter that does not start where the remainder is known does not fold.
#[test]
fn test_a_constant_amount_and_an_unknown_start_stay() {
    let constant = after(&counter("  %r = urem i32 %i, 10\n  store i32 %r, ptr %p"));
    assert!(constant.contains("urem"), "{constant}");
    let unknown = after(
        "define void @f(i32 %n, i32 %m, i32 %s, ptr %p) {
entry:
  br label %loop

loop:
  %i = phi i32 [ %s, %entry ], [ %next, %loop ]
  %r = urem i32 %i, %m
  store i32 %r, ptr %p
  %next = add nuw i32 %i, 1
  %c = icmp ult i32 %next, %n
  br i1 %c, label %loop, label %exit

exit:
  ret void
}
",
    );
    assert!(unknown.contains("urem"), "{unknown}");
}

/// A step of two, or an add that may wrap, is not a unit counter.
#[test]
fn test_a_step_of_two_and_a_wrapping_add_stay() {
    let two = after(
        "define void @f(i32 %n, i32 %m, ptr %p) {
entry:
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %r = urem i32 %i, %m
  store i32 %r, ptr %p
  %next = add nuw i32 %i, 2
  %c = icmp ult i32 %next, %n
  br i1 %c, label %loop, label %exit

exit:
  ret void
}
",
    );
    assert!(two.contains("urem"), "{two}");
    let wraps = after(
        "define void @f(i32 %n, i32 %m, ptr %p) {
entry:
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %r = urem i32 %i, %m
  store i32 %r, ptr %p
  %next = add i32 %i, 1
  %c = icmp ult i32 %next, %n
  br i1 %c, label %loop, label %exit

exit:
  ret void
}
",
    );
    assert!(wraps.contains("urem"), "{wraps}");
}
