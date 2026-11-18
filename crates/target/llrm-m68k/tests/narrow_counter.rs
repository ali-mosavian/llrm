//! A loop counter only its step and exit test read steps by a word.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

const DATALAYOUT: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

fn row(result: &str) -> String {
    format!(
        "{DATALAYOUT}@tile = global [4096 x i16] zeroinitializer, align 2
define i32 @row(i32 %y) {{
  %o = shl i32 %y, 6
  %base = getelementptr i16, ptr @tile, i32 %o
  br label %loop
loop:
  %i = phi i32 [ 0, %0 ], [ %i.next, %loop ]
  %sum = phi i32 [ 0, %0 ], [ %add, %loop ]
  %p = getelementptr i16, ptr %base, i32 %i
  %v = load i16, ptr %p, align 2
  %w = sext i16 %v to i32
  %add = add nsw i32 %sum, %w
  %i.next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %i.next, 40
  br i1 %done, label %exit, label %loop
exit:
  ret i32 {result}
}}
"
    )
}

/// tile's inner loop kept `addq.l #1,%d0; bne`: 8 clk a trip on the 68000 where
/// `addq.w` is 4, 34 clk against gcc's 32.
#[test]
fn a_counter_nothing_else_reads_steps_by_a_word() {
    let text = assembly(&row("%add"), Cpu::M68000, Level::O2).expect("compiles");
    assert!(text.contains("addq.w\t#1,") || text.contains("subq.w\t#"), "the counter steps by a long:\n{text}");
    assert!(!text.contains("addq.l\t#1,") && !text.contains("subq.l\t#"), "a long step is left:\n{text}");
}

/// A counter read after the loop keeps its upper half.
#[test]
fn a_counter_the_function_returns_stays_long() {
    let text = assembly(&row("%i.next"), Cpu::M68000, Level::O2).expect("compiles");
    assert!(!text.contains("addq.w\t#1,") && !text.contains("subq.w\t#"), "the returned counter was narrowed:\n{text}");
}
