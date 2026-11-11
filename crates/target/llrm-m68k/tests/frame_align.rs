//! The frame was rounded to the pin's 8-byte stack alignment, so a function
//! that only pushes two words and calls paid `lea (-4,%sp),%sp` and a later
//! `addq.l #4,%sp` (4 B and 6 clk at the 68000) that gcc, with m68k Linux's
//! 4-byte boundary, does not.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const CALL: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare i32 @g(i32, i32)
define i32 @pc(ptr %p, i32 %y) {
  %v = load i32, ptr %p
  %r = call i32 @g(i32 %v, i32 %y)
  ret i32 %r
}";

#[test]
fn a_frame_with_nothing_in_it_is_not_padded_for_the_stack_alignment() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O2, Level::Os] {
            let out = assembly(CALL, cpu_of(cpu).unwrap(), level).expect("compiles");
            assert!(
                !out.contains("lea\t(-4,%sp)") && !out.contains("addq.l\t#4, %sp"),
                "{cpu}: a padded frame:\n{out}"
            );
        }
    }
}
