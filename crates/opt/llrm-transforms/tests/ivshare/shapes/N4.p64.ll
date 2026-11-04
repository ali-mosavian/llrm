target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"

define void @f(ptr %p0, ptr %p1) {
entry:
  br label %loop
loop:
  %i = phi i8 [ 0, %entry ], [ %next, %loop ]
  %e0 = zext i8 %i to i64
  %a0 = getelementptr i8, ptr %p0, i64 %e0
  %v0 = load i8, ptr %a0
  %w0 = sext i8 %v0 to i32
  %e1 = zext i8 %i to i64
  %a1 = getelementptr i8, ptr %p1, i64 %e1
  %s0 = add i32 0, %w0
  %t1 = trunc i32 %s0 to i8
  store i8 %t1, ptr %a1
  %next = add nuw i8 %i, 1
  %c = icmp eq i8 %next, -6
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
