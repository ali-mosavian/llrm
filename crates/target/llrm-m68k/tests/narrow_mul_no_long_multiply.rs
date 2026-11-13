//! A 16-bit product of a loop counter, at the 68000 and 68010, in a size build
//! or at -O1: `jsr __mulsi3` and its 54 B routine where `mulu` does it (sieve
//! `i * i`: +54 B over gcc at -Os, -O1 +2 cells). The range the exit test gives
//! the counter is not in the IR's operands, so the selector's known bits never
//! saw halves; the narrowing by range (NRW-2) ran at -O2 and -O3 only.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

/// `for (i = 2; i < n; i++) sum += i * i` for an `unsigned short n`: the
/// counter is below 65536.
const LOOP: &str = "define i32 @f(i16 %n) {
entry:
  %bound = zext i16 %n to i32
  br label %loop
loop:
  %i = phi i32 [ 2, %entry ], [ %next, %loop ]
  %sum = phi i32 [ 0, %entry ], [ %add, %loop ]
  %sq = mul nuw nsw i32 %i, %i
  %add = add i32 %sum, %sq
  %next = add nuw nsw i32 %i, 1
  %done = icmp uge i32 %next, %bound
  br i1 %done, label %exit, label %loop
exit:
  ret i32 %add
}";

fn text(
    cpu: &str,
    level: Level,
) -> String {
    assembly(&format!("target datalayout = \"{DATALAYOUT}\"\n{LOOP}"), cpu_of(cpu).unwrap(), level).expect("compiles")
}

#[test]
fn a_counter_product_is_a_word_multiply_where_the_long_multiply_is_a_call() {
    for cpu in ["68000", "68010"] {
        for level in [Level::O1, Level::Os] {
            let out = text(cpu, level);
            assert!(!out.contains("__mulsi3"), "{cpu} {level:?}: the multiply is a call:\n{out}");
            assert!(out.contains("mulu"), "{cpu} {level:?}: no word multiply:\n{out}");
        }
    }
}

#[test]
fn a_size_build_at_the_68020_keeps_the_long_multiply() {
    let out = text("68020", Level::Os);
    assert!(!out.contains("__mulsi3") && !out.contains("mulu.w"), "{out}");
}
