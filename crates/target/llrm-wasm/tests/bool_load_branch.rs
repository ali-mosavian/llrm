//! A branch on the not of a loaded `i1`: the byte load promoted from `i1` is
//! known to hold 0 or 1 (`LegalizeLoadOps`' `AssertZext`), so `x != 1` is `x ==
//! 0`, one `i32.eqz`.

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

const LOOP: &str = "@bad = external global i1
declare i32 @next()
declare void @sink(i32)
define void @f() {
entry:
  br label %loop
loop:
  %n = call i32 @next()
  %c = icmp eq i32 %n, 255
  br i1 %c, label %exit, label %check
check:
  %b = load i1, ptr @bad
  br i1 %b, label %exit, label %loop
exit:
  call void @sink(i32 1)
  ret void
}
";

/// jpeg_fixed and jpeg_single -Os ran `i32.const 1; i32.ne` where the pin runs
/// `i32.eqz`: 6 instructions over the pin's count, which the cells' bars read.
#[test]
fn the_not_of_a_loaded_boolean_is_a_compare_with_zero() {
    let text = assembly(LOOP, Width::W32, Options { level: Level::Os, ..Options::default() }).unwrap();
    assert!(text.contains("i32.eqz"), "{text}");
    assert!(!text.contains("i32.ne"), "{text}");
}
