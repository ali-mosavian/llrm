target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

@back = global [2000 x i16] zeroinitializer

define i64 @f() {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %s = phi i64 [ 0, %entry ], [ %sum, %loop ]
  %p = getelementptr inbounds [2000 x i16], ptr @back, i64 0, i64 %i
  %v = load i16, ptr %p, align 2
  %x = sext i16 %v to i64
  %low = and i64 %i, 15
  %w = add nuw nsw i64 %low, 1
  %m = mul nsw i64 %w, %x
  %sum = add nsw i64 %m, %s
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 2000
  br i1 %done, label %exit, label %loop
exit:
  %r = phi i64 [ %sum, %loop ]
  ret i64 %r
}
