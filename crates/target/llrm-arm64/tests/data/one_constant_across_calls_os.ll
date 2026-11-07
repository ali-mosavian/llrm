target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
declare i32 @w(i32, ptr, i64)
define i32 @f(ptr %p, i32 %n) optsize {
entry:
  %s = shl i32 1, %n
  %a = call i32 @w(i32 1, ptr %p, i64 3)
  %c = icmp sgt i32 %a, 0
  br i1 %c, label %b1, label %b2
b1:
  %b = call i32 @w(i32 1, ptr %p, i64 7)
  %s1 = shl i32 1, %b
  %c2 = icmp sgt i32 %b, %n
  br i1 %c2, label %b3, label %b2
b2:
  %d = call i32 @w(i32 1, ptr %p, i64 9)
  %s2 = shl i32 1, %d
  br label %b3
b3:
  %x = phi i32 [ %s1, %b1 ], [ %s2, %b2 ]
  %e = call i32 @w(i32 1, ptr %p, i64 11)
  %t = add i32 %x, %e
  %u = add i32 %t, %s
  %v = call i32 @w(i32 1, ptr %p, i64 12)
  %r = add i32 %u, %v
  ret i32 %r
}
