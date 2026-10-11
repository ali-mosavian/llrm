use std::rc::Rc;

use llrm_mir::passes::PassManager;

use super::Pinned;
use crate::testing::{parsed, printed};

fn pinned(text: &str) -> String {
    let mut module = parsed(text);
    let mut manager = PassManager::default();
    manager.add(Pinned);
    manager.run_module(&mut module, Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    printed(&module)
}

/// The counter's step leaves a loop that also breaks on `i + 1 < 32` false, so
/// the exit phi reads 32: it read the stepped counter, which kept the old one
/// alive past its step (mandel: `lea; or; je; mov` where `inc; je` would do).
#[test]
fn test_an_exit_phi_of_a_stepped_counter_the_test_pins_is_that_constant() {
    let text = pinned(
        "define i32 @f(i32 %n) {
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp sgt i32 %i, %n
  br i1 %c, label %b3, label %b2

b2:
  %next = add i32 %i, 1
  %again = icmp slt i32 %next, 32
  br i1 %again, label %b1, label %b4

b3:
  ret i32 -1

b4:
  %last = phi i32 [ %next, %b2 ]
  ret i32 %last
}
",
    );
    assert!(text.contains("phi i32 [ 32, %b2 ]"), "{text}");
}

/// A value the edge leaves a range of more than one number is kept.
#[test]
fn test_an_exit_phi_the_test_does_not_pin_keeps_its_value() {
    let text = pinned(
        "define i32 @f(i32 %n) {
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %next, %b2 ]
  br label %b2

b2:
  %next = add i32 %i, 1
  %again = icmp slt i32 %next, %n
  br i1 %again, label %b1, label %b4

b4:
  %last = phi i32 [ %next, %b2 ]
  ret i32 %last
}
",
    );
    assert!(text.contains("phi i32 [ %next, %b2 ]"), "{text}");
}
