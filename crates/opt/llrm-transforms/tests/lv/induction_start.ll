target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
define void @f(ptr noalias %a, i64 %s) {
entry:
  br label %loop
loop:
  %i = phi i64 [ %s, %entry ], [ %next, %loop ]
  %t = trunc i64 %i to i16
  %p = getelementptr inbounds i16, ptr %a, i64 %i
  store i16 %t, ptr %p, align 2
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 2000
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
