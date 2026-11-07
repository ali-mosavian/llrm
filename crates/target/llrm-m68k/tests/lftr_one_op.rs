//! A loop `for (i = 0; i + 1 < n - 1; i++)` in a function built for size kept
//! its up counter (`addq.l #1; cmp.l; blt`): the optsize LFTR budget was 0, so
//! the exit limit `n - 1`, one operation, was refused. partition -Os was 4 B
//! over the same loop at -O2, memset 2 B. The count-down (`subq.l #1; bne`)
//! needs the limit to cost no more than one operation.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @f(ptr %a, i32 %n) optsize {
entry:
  %m = add nsw i32 %n, -1
  %c = icmp sgt i32 %n, 1
  br i1 %c, label %loop, label %out
loop:
  %i = phi i32 [ 0, %entry ], [ %i1, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s1, %loop ]
  %p = getelementptr inbounds i32, ptr %a, i32 %i
  %v = load i32, ptr %p
  %s1 = add i32 %s, %v
  %i1 = add nuw nsw i32 %i, 1
  %e = icmp slt i32 %i1, %m
  br i1 %e, label %loop, label %out
out:
  %r = phi i32 [ 0, %entry ], [ %s1, %loop ]
  %t = add i32 %r, %m
  ret i32 %t
}
";

#[test]
fn a_size_loop_whose_limit_costs_one_operation_counts_down() {
    for cpu in ["68000", "68020"] {
        let out = assembly(IR, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        let text: Vec<&str> = out.lines().map(str::trim).collect();
        assert!(text.iter().any(|line| line.starts_with("bne")), "{cpu}: no count-down branch:\n{out}");
        assert!(!text.iter().any(|line| line.starts_with("addq.l\t#1")), "{cpu}: the up counter is kept:\n{out}");
    }
}
