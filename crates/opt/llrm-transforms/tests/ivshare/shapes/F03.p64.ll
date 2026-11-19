target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"

define void @f(ptr %p0, ptr %p1, i64 %n) {
entry:
  %g = icmp sgt i64 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %a0 = getelementptr i8, ptr %p0, i64 %i
  %v0 = load i8, ptr %a0
  %w0 = sext i8 %v0 to i32
  %a1 = getelementptr i8, ptr %p1, i64 %i
  %s0 = add i32 0, %w0
  %t1 = trunc i32 %s0 to i8
  store i8 %t1, ptr %a1
  %next = add nuw i64 %i, 1
  %c = icmp eq i64 %next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
