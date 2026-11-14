target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
@s = global [4000 x i8] zeroinitializer, align 1
define i64 @f() {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %in, %loop ]
  %acc = phi i64 [ 0, %entry ], [ %next, %loop ]
  %p = getelementptr inbounds i8, ptr @s, i64 %i
  %v = load i8, ptr %p, align 1
  %e = zext i8 %v to i64
  %next = add nuw nsw i64 %acc, %e
  %in = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %in, 4000
  br i1 %done, label %exit, label %loop
exit:
  %r = phi i64 [ %next, %loop ]
  ret i64 %r
}
