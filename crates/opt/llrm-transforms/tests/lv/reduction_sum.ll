target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
@s = global [2000 x i32] zeroinitializer, align 4
define i32 @f(i32 %start) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %in, %loop ]
  %acc = phi i32 [ %start, %entry ], [ %next, %loop ]
  %p = getelementptr inbounds i32, ptr @s, i64 %i
  %v = load i32, ptr %p, align 4
  %next = add nsw i32 %acc, %v
  %in = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %in, 1003
  br i1 %done, label %exit, label %loop
exit:
  %r = phi i32 [ %next, %loop ]
  ret i32 %r
}
