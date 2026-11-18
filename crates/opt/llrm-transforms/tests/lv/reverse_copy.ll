target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
define void @f(ptr noalias %a) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 1919, %entry ], [ %next, %loop ]
  %p = getelementptr inbounds i16, ptr %a, i64 %i
  %v = load i16, ptr %p, align 2
  %q = getelementptr inbounds i8, ptr %p, i64 160
  store i16 %v, ptr %q, align 2
  %next = add nsw i64 %i, -1
  %done = icmp eq i64 %i, 0
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
