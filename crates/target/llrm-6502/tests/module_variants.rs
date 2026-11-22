//! The routine variants a module chooses (a cold long division takes the small
//! wide path, a hot multiply the speed routine) hold together: a function's
//! text written again for the multiply dropped the division's choice, linking
//! the wide path (+141 B) for one cold division (compound_assignment_types at
//! -O2).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

const IR: &str = "define i32 @hot(ptr %p, i32 %a, i32 %b, i32 %n) noinline nounwind {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %v = load volatile i32, ptr %p
  %m = mul i32 %a, %v
  store volatile i32 %m, ptr %p
  %next = add i32 %i, 1
  %done = icmp eq i32 %next, %n
  br i1 %done, label %exit, label %loop
exit:
  %q = sdiv i32 %n, %a
  ret i32 %q
}
define i32 @cold(i32 %x, i32 %y) noinline nounwind {
  %q = sdiv i32 %x, %y
  ret i32 %q
}
@v = global i32 3
define i32 @main(ptr %p) nounwind {
  %x = load volatile i32, ptr @v
  %y = load volatile i32, ptr @v
  %a = call i32 @hot(ptr %p, i32 %x, i32 %y, i32 5)
  %b = call i32 @cold(i32 %x, i32 %y)
  %c = add i32 %a, %b
  ret i32 %c
}\n";

#[test]
fn a_hot_multiply_keeps_the_cold_division_small() {
    let ir = format!("target datalayout = \"{}\"\n{IR}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("__mulsi3_fast"), "{text}");
    assert!(text.contains("__divsi3_fast_lean"), "{text}");
    assert!(!text.contains("__divsi3_fast\n"), "{text}");
}
