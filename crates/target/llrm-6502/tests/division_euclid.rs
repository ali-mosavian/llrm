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

/// -Os takes the speed 16-bit division where the share of the module's clocks
/// it saves is at least twice the share of its bytes it adds (`[share.size]
/// division16`): a hot remainder loop among 360 B of other code.
fn hot_rem_loop_with_code(code_stores: usize) -> String {
    let mut ir = String::new();
    for i in 0..code_stores {
        ir += &format!("@g{i} = global i16 0\n");
    }
    ir += "define void @ballast() noinline {\n";
    for i in 0..code_stores {
        ir += &format!("  store volatile i16 {}, ptr @g{i}\n", 1000 + i * 7);
    }
    ir += "  ret void\n}\n";
    ir += "define i16 @f(i16 %b) {
entry:
  call void @ballast()
  %d = or i16 %b, 1
  br label %l
l:
  %i = phi i16 [ 0, %entry ], [ %in, %l ]
  %s = phi i16 [ 0, %entry ], [ %sn, %l ]
  %x = mul i16 %i, 37
  %r = urem i16 %x, %d
  %sn = add i16 %s, %r
  %in = add i16 %i, 1
  %c = icmp ult i16 %in, 1000
  br i1 %c, label %l, label %done
done:
  ret i16 %sn
}
";
    format!("target datalayout = \"{}\"\n{ir}", datalayout())
}

/// div_constants_short -Os ran 76.3M clocks against llvm-mos's 65.6M with the
/// compact remainder in 1000-trip loops: a module whose loop dominates its
/// clocks takes the speed routine for its 131 B.
#[test]
fn a_hot_remainder_loop_among_other_code_calls_the_speed_routine_at_os() {
    let text = assembly(&hot_rem_loop_with_code(60), &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("__umodhi3\n") && !text.contains("__umodhi3c"), "the speed remainder at -Os:\n{text}");
}

/// The same loop alone in its module: the speed routine's 131 B are most of
/// the module's bytes, so its share is not twice the clocks it saves. The
/// compact routine stays (bytes first at -Os).
#[test]
fn a_remainder_loop_that_is_the_whole_module_keeps_the_compact_routine_at_os() {
    let text = assembly(&hot_rem_loop_with_code(0), &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("__umodhi3c\n"), "the compact remainder at -Os:\n{text}");
}

/// idiv_loop_os at -Os ran 2.31M clocks against llvm-mos's 1.07M on the compact
/// remainder: the share rule (64% of the clocks for its 32% of the bytes)
/// refused a speed routine whose image (229 B) is under the 285 B the pin links
/// for a signed remainder (`[pin.division16]`). Row 6502-DIV-16-OS-IDIV-LOOP.
#[test]
fn the_euclid_loop_takes_the_speed_routine_at_os_within_the_pins_image() {
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
