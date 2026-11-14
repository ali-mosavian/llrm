target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
@s = global [2000 x i16] zeroinitializer, align 2
define i64 @f() {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %in, %loop ]
  %acc = phi i64 [ 0, %entry ], [ %next, %loop ]
  %p = getelementptr inbounds [2 x i8], ptr @s, i64 %i
  %v = load i16, ptr %p, align 2
  %w = sext i16 %v to i64
  %m = and i64 %i, 15
  %k = add nuw nsw i64 %m, 1
  %t = mul nsw i64 %k, %w
  %next = add nsw i64 %t, %acc
  %in = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %in, 2000
  br i1 %done, label %exit, label %loop
exit:
  %r = phi i64 [ %next, %loop ]
  ret i64 %r
}
