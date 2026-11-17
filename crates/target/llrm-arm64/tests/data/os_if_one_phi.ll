target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

define i64 @g(ptr %p, i64 %n) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %i1, %join ]
  %x = phi i64 [ 0, %entry ], [ %x2, %join ]
  %y = phi i64 [ 1, %entry ], [ %y, %join ]
  %a = getelementptr i8, ptr %p, i64 %i
  %c = load i8, ptr %a
  %z = icmp eq i8 %c, 0
  br i1 %z, label %then, label %join
then:
  %w = zext i8 %c to i64
  %t = add i64 %x, %i
  %u = mul i64 %y, %i
  br label %join
join:
  %x2 = phi i64 [ %x, %loop ], [ %t, %then ]
  %i1 = add i64 %i, 1
  %d = icmp eq i64 %i1, %n
  br i1 %d, label %exit, label %loop
exit:
  %r = add i64 %x2, %y
  ret i64 %r
}
