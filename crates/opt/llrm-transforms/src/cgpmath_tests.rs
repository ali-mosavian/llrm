//! A compare of a sum or a difference with an operand becomes the overflow
//! intrinsic that yields both.

use crate::cgpaddr::CgpAddr;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"\n\n";

fn after(text: &str) -> String {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(CgpAddr);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    printed(&module)
}

/// `add %x, -32` beside `icmp ult %x, 32`, as LSR leaves a counted-down loop:
/// the pin forms `usub.with.overflow` (`sub; jae` on x86). Without the combine
/// the loop kept `lea; cmp; ja`, +5% cycles on shellsort.
#[test]
fn test_a_compare_with_the_add_of_its_negated_constant_is_a_usub_with_overflow() {
    let text = after(
        "define i16 @f(i16 %x, ptr %p) {
b0:
  %s = add i16 %x, -32
  store i16 %s, ptr %p
  %c = icmp ult i16 %x, 32
  br i1 %c, label %y, label %n

y:
  ret i16 1

n:
  ret i16 0
}
",
    );
    assert!(text.contains("@llvm.usub.with.overflow.i16(i16 %x, i16 32)"), "{text}");
    assert!(text.contains("extractvalue"), "{text}");
    assert!(!text.contains("icmp ult i16 %x, 32"), "{text}");
    assert!(!text.contains("add i16 %x, -32"), "{text}");
}

#[test]
fn test_a_sum_below_its_operand_is_a_uadd_with_overflow() {
    let text = after(
        "define i16 @f(i16 %a, i16 %b, ptr %p) {
b0:
  %s = add i16 %a, %b
  store i16 %s, ptr %p
  %c = icmp ult i16 %s, %a
  br i1 %c, label %y, label %n

y:
  ret i16 1

n:
  ret i16 0
}
",
    );
    assert!(text.contains("@llvm.uadd.with.overflow.i16(i16 %a, i16 %b)"), "{text}");
    assert!(!text.contains("icmp ult"), "{text}");
}

#[test]
fn test_a_difference_below_nothing_stays() {
    let text = after(
        "define i16 @f(i16 %x, i16 %y, ptr %p) {
b0:
  %s = add i16 %x, -32
  store i16 %s, ptr %p
  %c = icmp ult i16 %x, %y
  br i1 %c, label %t, label %n

t:
  ret i16 1

n:
  ret i16 0
}
",
    );
    assert!(!text.contains("with.overflow"), "{text}");
}

/// The math in another block is left alone, but a loop counter's increment
/// moves up to the compare that sits in the loop above it.
#[test]
fn test_a_counters_decrement_in_the_latch_moves_to_the_compare_in_the_header() {
    let text = after(
        "define void @f(i16 %n, ptr %p) {
entry:
  br label %loop

loop:
  %i = phi i16 [ %n, %entry ], [ %next, %body ]
  %c = icmp ult i16 %i, 32
  br i1 %c, label %exit, label %body

body:
  store i16 %i, ptr %p
  %next = add i16 %i, -32
  br label %loop

exit:
  ret void
}
",
    );
    assert!(text.contains("@llvm.usub.with.overflow.i16(i16 %i, i16 32)"), "{text}");
    assert!(!text.contains("add i16 %i, -32"), "{text}");
}

#[test]
fn test_a_difference_in_another_block_that_is_no_counter_stays() {
    let text = after(
        "define i16 @f(i16 %x, ptr %p) {
b0:
  %c = icmp ult i16 %x, 32
  br i1 %c, label %t, label %n

t:
  %s = add i16 %x, -32
  store i16 %s, ptr %p
  store i16 %s, ptr %p
  ret i16 1

n:
  ret i16 0
}
",
    );
    assert!(text.contains("icmp ult i16 %x, 32"), "{text}");
}
