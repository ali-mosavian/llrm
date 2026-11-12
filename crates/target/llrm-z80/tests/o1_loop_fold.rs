//! A loop that runs more than once keeps its guard and latch as one test at
//! -O1/-O2/-O3.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const IR: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"
declare void @report(i16)
define void @run() {
entry:
  br label %outer
outer:
  %t = phi i16 [ 0, %entry ], [ %t1, %next ]
  br label %body
body:
  %i = phi i16 [ 0, %outer ], [ %i1, %body ]
  call void @report(i16 %i)
  %i1 = add i16 %i, 1
  %c = icmp ne i16 %i1, 64
  br i1 %c, label %body, label %next
next:
  %t1 = add i16 %t, 1
  %d = icmp ne i16 %t1, 6
  br i1 %d, label %outer, label %exit
exit:
  ret void
}
";

/// pattern_fill -O1 closed its 64-trip loop by `dec b; jr z; push bc; jp top`
/// (442 B, 308920 T; SDCC 440 B) where the merged guard and latch close it by
/// `djnz`/`jr nz` (437 B, 305848 T).
#[test]
fn a_loop_that_repeats_closes_by_a_conditional_jump_not_a_jp() {
    for level in [Level::O1, Level::O2, Level::O3] {
        let out = text(IR, "run", 0, &Request::new(level)).expect("compiles");
        assert!(!out.lines().any(|l| l.trim_start().starts_with("jp L0_")), "{level:?}:\n{out}");
    }
}
