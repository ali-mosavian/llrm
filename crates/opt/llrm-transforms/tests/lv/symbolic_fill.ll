target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

define void @f(ptr noalias %d, i64 %v, i64 %a, i64 %b) {
entry:
  %enter = icmp slt i64 %a, %b
  br i1 %enter, label %loop, label %exit
loop:
  %i = phi i64 [ %next, %loop ], [ %a, %entry ]
  %p = getelementptr inbounds i64, ptr %d, i64 %i
  store i64 %v, ptr %p, align 8
  %next = add nsw i64 %i, 1
  %done = icmp eq i64 %next, %b
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
