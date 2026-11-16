target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@table = constant [3 x i32] [i32 7, i32 8, i32 9]

define void @f(ptr %p) {
  %a = load <4 x i32>, ptr %p
  %r = add <4 x i32> %a, <i32 1, i32 2, i32 3, i32 4>
  store <4 x i32> %r, ptr %p
  ret void
}

define double @g(double %x) {
  %r = fadd double %x, 3.14159
  ret double %r
}

define i32 @h(i64 %i) {
  %p = getelementptr [3 x i32], ptr @table, i64 0, i64 %i
  %v = load i32, ptr %p
  ret i32 %v
}
