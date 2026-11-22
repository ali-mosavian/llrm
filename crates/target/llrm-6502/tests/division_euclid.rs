//! A divide whose quotient is a few bits (the steps of Euclid's algorithm, a
//! ratio near 1) walked 16 steps of a bit loop: idiv_loop_os at -O2 ran 2.3M
//! clocks against llvm-mos's 1.07M. The speed `__udivhi3` shifts the divisor up
//! to the dividend and takes a step a quotient bit (compiler-rt's
//! `__udivmodhi4`), and a module whose divisions are hot links it at the speed
//! levels.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

/// idiv_loop_os (`tests/run/c`): gcd by recursion over `%`, summed over 300
/// pairs.
const GCD_SUM: &str = "declare void @report(i32)
define internal i16 @gcd(i16 %a, i16 %b) {
entry:
  %z = icmp eq i16 %b, 0
  br i1 %z, label %done, label %rec
rec:
  %r = srem i16 %a, %b
  %g = call i16 @gcd(i16 %b, i16 %r)
  ret i16 %g
done:
  ret i16 %a
}
define i32 @total(i16 %n) {
entry:
  br label %l
l:
  %i = phi i16 [ 1, %entry ], [ %in, %l ]
  %s = phi i32 [ 0, %entry ], [ %sn, %l ]
  %x = mul i16 %i, 7
  %x3 = add i16 %x, 3
  %y = mul i16 %i, 5
  %y2 = add i16 %y, 2
  %g1 = call i16 @gcd(i16 %x3, i16 %y2)
  %j = add i16 %i, 90
  %g2 = call i16 @gcd(i16 %j, i16 360)
  %gs = add i16 %g1, %g2
  %ge = sext i16 %gs to i32
  %sn = add i32 %s, %ge
  %in = add i16 %i, 1
  %c = icmp sle i16 %in, %n
  br i1 %c, label %l, label %done
done:
  ret i32 %sn
}
define i16 @main() {
  %t = call i32 @total(i16 300)
  call void @report(i32 %t)
  ret i16 0
}
";

#[test]
fn a_hot_remainder_loop_calls_the_speed_routine_at_o2() {
    let ir = format!("target datalayout = \"{}\"\n{GCD_SUM}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("__modhi3\n") && !text.contains("__modhi3c"), "the speed remainder at -O2:\n{text}");
}

/// The same program at -Os called the compact `__modhi3c` (idiv_loop_os 2.31M
/// clocks against llvm-mos's 1.07M): the speed routine is the module's choice
/// at size by the share of its clocks it saves against the share of its bytes
/// it adds.
#[test]
fn a_hot_remainder_loop_calls_the_speed_routine_at_os() {
    let ir = format!("target datalayout = \"{}\"\n{GCD_SUM}", datalayout());
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("__modhi3\n") && !text.contains("__modhi3c"), "the speed remainder at -Os:\n{text}");
}

/// A division that runs once repays no byte: -Os keeps the compact routine.
#[test]
fn a_cold_remainder_keeps_the_compact_routine_at_os() {
    let ir = format!(
        "target datalayout = \"{}\"\ndefine i16 @once(i16 %a, i16 %b) {{\n  %r = srem i16 %a, %b\n  ret i16 %r\n}}\n",
        datalayout()
    );
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("__modhi3c\n"), "the compact remainder at -Os:\n{text}");
}
