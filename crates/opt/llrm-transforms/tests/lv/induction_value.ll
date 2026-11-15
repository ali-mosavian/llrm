target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
define void @f(ptr noalias %a) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %v = add i64 %i, 5
  %p = getelementptr inbounds i64, ptr %a, i64 %i
  store i64 %v, ptr %p, align 8
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 2000
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
