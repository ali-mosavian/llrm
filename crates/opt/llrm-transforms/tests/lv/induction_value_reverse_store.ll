target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
define void @f(ptr noalias %a) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 1999, %entry ], [ %next, %loop ]
  %p = getelementptr inbounds i32, ptr %a, i32 %i
  store i32 %i, ptr %p, align 4
  %next = add nsw i32 %i, -1
  %done = icmp eq i32 %i, 0
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
