target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
declare void @use(ptr)
define i64 @f(i16 %x) {
  %s = alloca [64 x i16], align 4
  %b = alloca [64 x i64], align 8
  %p0 = getelementptr inbounds i16, ptr %s, i64 0
  store i16 %x, ptr %p0, align 2
  %p1 = getelementptr inbounds i16, ptr %s, i64 1
  store i16 %x, ptr %p1, align 2
  %p2 = getelementptr inbounds i16, ptr %s, i64 2
  store i16 %x, ptr %p2, align 2
  %p3 = getelementptr inbounds i16, ptr %s, i64 3
  store i16 %x, ptr %p3, align 2
  %p4 = getelementptr inbounds i16, ptr %s, i64 4
  store i16 %x, ptr %p4, align 2
  %p5 = getelementptr inbounds i16, ptr %s, i64 5
  store i16 %x, ptr %p5, align 2
  %p6 = getelementptr inbounds i16, ptr %s, i64 6
  store i16 %x, ptr %p6, align 2
  %p7 = getelementptr inbounds i16, ptr %s, i64 7
  store i16 %x, ptr %p7, align 2
  call void @use(ptr %s)
  call void @use(ptr %b)
  %q0 = getelementptr inbounds i64, ptr %b, i64 60
  %q1 = getelementptr inbounds i64, ptr %b, i64 61
  %l0 = load i64, ptr %q0, align 8
  %l1 = load i64, ptr %q1, align 8
  %r = add i64 %l0, %l1
  ret i64 %r
}
