target triple = "aarch64-unknown-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
define i64 @f(ptr %p, i64 %a, i64 %b, i64 %c) {
  %v = load i8, ptr %p, align 1
  %z = zext i8 %v to i64
  %cmp = icmp eq i64 %z, 0
  %s = shl i64 %a, 1
  %t = add i64 %s, %z
  %u = add i64 %b, %t
  %m = and i64 %u, 536870911
  %r = select i1 %cmp, i64 %c, i64 %m
  ret i64 %r
}
