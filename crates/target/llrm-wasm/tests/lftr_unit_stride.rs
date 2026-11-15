//! The pin's `linearFunctionTestReplace` takes a unit-stride counter only; a
//! step-7 loop keeps its `ult` exit and `LSR` rebases the counter. Rewriting it
//! to `ne 259` left the body on the old counter and the exit on the next one:
//! three locals and one more instruction per iteration (`byte_multiply` -O1,
//! 26 against the pin's 25 per inner iteration; `div8_os` -O1 +1.87%).

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

const STEP_SEVEN: &str = "declare i32 @g(i32, i32)
define i32 @f(i32 %a) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %n, %loop ]
  %s = phi i32 [ 0, %entry ], [ %t, %loop ]
  %m = call i32 @g(i32 %i, i32 %s)
  %t = add i32 %s, %m
  %n = add nuw nsw i32 %i, 7
  %c = icmp ult i32 %n, 256
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %t
}
";

#[test]
fn a_step_seven_loop_keeps_its_ult_exit() {
    let text = assembly(STEP_SEVEN, Width::W32, Options { level: Level::O1, ..Options::default() }).unwrap();
    assert!(text.contains("i32.lt_u"), "{text}");
    assert!(!text.contains("i32.ne"), "{text}");
}
