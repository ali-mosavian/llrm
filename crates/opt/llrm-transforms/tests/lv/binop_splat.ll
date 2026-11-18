target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

define void @f(ptr noalias %a, ptr noalias %b, ptr noalias %c) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %pa = getelementptr inbounds i8, ptr %a, i64 %i
  %x = load i8, ptr %pa, align 1
  %r = add i8 %x, 5
  %pc = getelementptr inbounds i8, ptr %c, i64 %i
  store i8 %r, ptr %pc, align 1
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 1920
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
