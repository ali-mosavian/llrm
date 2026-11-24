target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
@k0 = global i32 3
@k1 = global i32 4
@k2 = global i32 5
@k3 = global i32 6
@k4 = global i32 7
@k5 = global i32 8
@k6 = global i32 9
@k7 = global i32 10
declare i32 @g(ptr, i32)
define i32 @f() minsize optsize {
  %buf = alloca [32 x i8], align 8
  %v0 = load i32, ptr @k0
  %v1 = load i32, ptr @k1
  %v2 = load i32, ptr @k2
  %v3 = load i32, ptr @k3
  %v4 = load i32, ptr @k4
  %v5 = load i32, ptr @k5
  %v6 = load i32, ptr @k6
  %v7 = load i32, ptr @k7
  %r0 = call i32 @g(ptr %buf, i32 %v0)
  %r1 = call i32 @g(ptr %buf, i32 %v1)
  %r2 = call i32 @g(ptr %buf, i32 %v2)
  %r3 = call i32 @g(ptr %buf, i32 %v3)
  %r4 = call i32 @g(ptr %buf, i32 %v4)
  %r5 = call i32 @g(ptr %buf, i32 %v5)
  %r6 = call i32 @g(ptr %buf, i32 %v6)
  %r7 = call i32 @g(ptr %buf, i32 %v7)
  %s0 = add i32 %r0, %r1
  %s1 = add i32 %s0, %r2
  %s2 = add i32 %s1, %r3
  %s3 = add i32 %s2, %r4
  %s4 = add i32 %s3, %r5
  %s5 = add i32 %s4, %r6
  %s6 = add i32 %s5, %r7
  ret i32 %s6
}
