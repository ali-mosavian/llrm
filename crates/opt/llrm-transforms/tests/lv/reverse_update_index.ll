target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
define void @f(ptr noalias %a, i64 %n) {
entry:
  %e = icmp eq i64 %n, 0
  br i1 %e, label %exit, label %loop
loop:
  %i = phi i64 [ %n, %entry ], [ %next, %loop ]
  %next = add nsw i64 %i, -1
  %p = getelementptr inbounds i8, ptr %a, i64 %next
  %v = load i8, ptr %p, align 1
  %q = getelementptr inbounds i8, ptr %p, i64 3
  store i8 %v, ptr %q, align 1
  %done = icmp eq i64 %next, 0
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
