target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
@t = global [40 x i16] zeroinitializer, align 2
define i64 @f() {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %acc = phi i64 [ 0, %entry ], [ %sum, %loop ]
  %p = getelementptr inbounds i16, ptr @t, i64 %i
  %v = load i16, ptr %p, align 2
  %e = sext i16 %v to i64
  %sum = add nsw i64 %acc, %e
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 40
  br i1 %done, label %exit, label %loop
exit:
  %r = phi i64 [ %sum, %loop ]
  ret i64 %r
}
