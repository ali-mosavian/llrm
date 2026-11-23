target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
declare void @g()
define void @f(ptr %p) {
  %q = getelementptr i8, ptr %p, i64 106
  %v = load volatile i8, ptr %q, align 1
  %c = icmp ne i8 %v, 0
  br i1 %c, label %a, label %b
a:
  call void @g()
  ret void
b:
  ret void
}
