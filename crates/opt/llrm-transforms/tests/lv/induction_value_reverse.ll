target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
define i32 @f() {
entry:
  br label %loop
loop:
  %i = phi i32 [ 1999, %entry ], [ %next, %loop ]
  %s = phi i32 [ 0, %entry ], [ %sum, %loop ]
  %t = mul i32 %i, 3
  %sum = add i32 %s, %t
  %next = add nsw i32 %i, -1
  %done = icmp eq i32 %i, 0
  br i1 %done, label %exit, label %loop
exit:
  %r = phi i32 [ %sum, %loop ]
  ret i32 %r
}
