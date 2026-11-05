//! A 16-trip loop of two calls was unrolled in full: 32 `jsr` where the loop
//! is 2 (tests/run/c fcmp_all -O2 68020: 37758 B against gcc's 5324, its
//! 48 compare loops unrolled). A trip's control is about 6 B of clocks at
//! 4 clk/B, so a body of calls does not pay for 15 copies.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const CALLS: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare void @f(i32)
declare void @g(i32)
define void @k() {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %n, %loop ]
  call void @f(i32 %i)
  call void @g(i32 %i)
  %n = add i32 %i, 1
  %c = icmp eq i32 %n, 16
  br i1 %c, label %done, label %loop
done:
  ret void
}
";

/// 4 clk/B: a hot loop of two instructions is still unrolled (crc16 -O2).
const TINY: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @k(i32 %x) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %n, %loop ]
  %a = phi i32 [ %x, %entry ], [ %b, %loop ]
  %b = xor i32 %a, %i
  %n = add i32 %i, 1
  %c = icmp eq i32 %n, 4
  br i1 %c, label %done, label %loop
done:
  ret i32 %b
}
";

fn calls(text: &str) -> usize {
    let out = assembly(text, cpu_of("68020").unwrap(), Level::O2).expect("compiles");
    out.lines().filter(|line| line.trim_start().starts_with("jsr")).count()
}

#[test]
fn a_loop_of_calls_stays_a_loop() {
    assert!(calls(CALLS) <= 4, "{} jsr", calls(CALLS));
}

#[test]
fn a_four_trip_loop_of_one_instruction_is_unrolled() {
    let out = assembly(TINY, cpu_of("68020").unwrap(), Level::O2).expect("compiles");
    assert!(!out.contains("LBB"), "{out}");
}
