//! A 32-bit multiply by a constant outside every loop is a call of `__mulsi3`
//! at -O2 when the module is fewer bytes so (the routine is linked once for
//! all of them): compound_assignment_types was 1,188 B at -O2 against 1,029 B
//! at -Os, `check * 7` expanded twelve times.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn asm(
    functions: &str,
    level: Level,
) -> String {
    let ir = format!("target datalayout = \"{}\"\n{functions}", datalayout());
    assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"))
}

/// Four `x * 7` in one block, each of a value the block stores.
const FOUR: &str = "define void @four(ptr %p, i32 %a, i32 %b, i32 %c, i32 %d) nounwind {
  %1 = mul i32 %a, 7
  %2 = mul i32 %b, 7
  %3 = mul i32 %c, 7
  %4 = mul i32 %d, 7
  store volatile i32 %1, ptr %p
  store volatile i32 %2, ptr %p
  store volatile i32 %3, ptr %p
  store volatile i32 %4, ptr %p
  ret void
}\n";

const ONE: &str = "define void @one(ptr %p, i32 %a) nounwind {
  %1 = mul i32 %a, 7
  store volatile i32 %1, ptr %p
  ret void
}\n";

/// The same four, each trip of a loop.
const IN_A_LOOP: &str = "define void @looped(ptr %p, ptr %q, i32 %n) nounwind {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %a = load volatile i32, ptr %q
  %b = load volatile i32, ptr %q
  %c = load volatile i32, ptr %q
  %d = load volatile i32, ptr %q
  %m1 = mul i32 %a, 7
  %m2 = mul i32 %b, 7
  %m3 = mul i32 %c, 7
  %m4 = mul i32 %d, 7
  store volatile i32 %m1, ptr %p
  store volatile i32 %m2, ptr %p
  store volatile i32 %m3, ptr %p
  store volatile i32 %m4, ptr %p
  %next = add i32 %i, 1
  %done = icmp eq i32 %next, %n
  br i1 %done, label %exit, label %loop
exit:
  ret void
}\n";

/// Four expansions of `x * 7` were 4 x 22 B left in line at -O2; the calls and
/// the routine are fewer bytes.
#[test]
fn four_constant_multiplies_outside_a_loop_are_calls_at_o2() {
    let text = asm(FOUR, Level::O2);
    assert!(text.contains("jsr __mulsi3"), "{text}");
}

/// One multiply does not pay for the routine: it stays shifts and adds.
#[test]
fn one_constant_multiply_stays_in_line() {
    let text = asm(ONE, Level::O2);
    assert!(!text.contains("__mulsi3"), "{text}");
}

/// A call is some 1,900 clocks a multiply: a place in a loop keeps the shifts.
#[test]
fn constant_multiplies_in_a_loop_stay_in_line() {
    let text = asm(IN_A_LOOP, Level::O2);
    assert!(!text.contains("__mulsi3"), "{text}");
}
