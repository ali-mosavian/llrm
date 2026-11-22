target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define i32 @sum(ptr %p, i32 %n) {
entry:
  %c = icmp sgt i32 %n, 0
  br i1 %c, label %ph, label %exit
ph:
  %wide = zext i32 %n to i64
  br label %loop
loop:
  %i = phi i64 [ 0, %ph ], [ %i.next, %loop ]
  %s = phi i32 [ 0, %ph ], [ %s.next, %loop ]
  %g = getelementptr inbounds i32, ptr %p, i64 %i
  %v = load i32, ptr %g, align 4
  %s.next = add i32 %s, %v
  %i.next = add nuw nsw i64 %i, 1
  %e = icmp eq i64 %i.next, %wide
  br i1 %e, label %exit, label %loop
exit:
  %r = phi i32 [ 0, %entry ], [ %s.next, %loop ]
  ret i32 %r
}
