target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"

define void @f(ptr %p0, ptr %p1, i64 %n) {
entry:
  %g = icmp sgt i64 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %x0 = mul nuw nsw i64 %i, 7
  %a0 = getelementptr i32, ptr %p0, i64 %x0
  %v0 = load i32, ptr %a0
  %a1 = getelementptr i32, ptr %p1, i64 %i
  %s0 = add i32 0, %v0
  store i32 %s0, ptr %a1
  %next = add nuw i64 %i, 1
  %c = icmp eq i64 %next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
