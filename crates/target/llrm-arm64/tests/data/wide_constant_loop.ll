target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"

define double @scale(ptr %p, i64 %n) {
entry:
  br label %loop

loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %acc = phi double [ 0.0, %entry ], [ %sum, %loop ]
  %q = getelementptr double, ptr %p, i64 %i
  %x = load double, ptr %q
  %y = fmul double %x, 0x3F847AE147AE147B
  %sum = fadd double %acc, %y
  %next = add i64 %i, 1
  %more = icmp ult i64 %next, %n
  br i1 %more, label %loop, label %exit

exit:
  ret double %sum
}
