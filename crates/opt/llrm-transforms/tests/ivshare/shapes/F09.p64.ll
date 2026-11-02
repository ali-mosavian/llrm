target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"

define void @f(ptr %p0, ptr %p1, ptr %p2, i64 %n) {
entry:
  %g = icmp sgt i64 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %a0 = getelementptr i64, ptr %p0, i64 %i
  %v0 = load i64, ptr %a0
  %w0 = trunc i64 %v0 to i32
  %a1 = getelementptr i16, ptr %p1, i64 %i
  %v1 = load i16, ptr %a1
  %w1 = sext i16 %v1 to i32
  %a2 = getelementptr i8, ptr %p2, i64 %i
  %s0 = add i32 0, %w0
  %s1 = add i32 %s0, %w1
  %t2 = trunc i32 %s1 to i8
  store i8 %t2, ptr %a2
  %next = add nuw i64 %i, 1
  %c = icmp eq i64 %next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
