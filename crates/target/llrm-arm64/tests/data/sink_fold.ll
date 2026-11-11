target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

declare i32 @use(ptr, i32)

define i32 @fold(ptr %p, i32 %n, i32 %k) {
entry:
  %idx = sext i32 %n to i64
  %next = add nsw i32 %n, 1
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %i2, %latch ]
  %hit = icmp eq i32 %i, %k
  br i1 %hit, label %call, label %latch

call:
  %q = getelementptr inbounds i32, ptr %p, i64 %idx
  store i32 %i, ptr %q, align 4
  %r = call i32 @use(ptr %p, i32 %next)
  br label %latch

latch:
  %i2 = add nsw i32 %i, 1
  %more = icmp slt i32 %i2, 100
  br i1 %more, label %loop, label %exit

exit:
  ret i32 0
}
