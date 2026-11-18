target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

declare void @ext(ptr, i32)

define internal void @op(ptr %s, ptr %f) alwaysinline optsize {
  call void %f(ptr %s)
  ret void
}

define internal void @w1(ptr %s) inlinehint optsize {
  call void @ext(ptr %s, i32 1)
  call void @ext(ptr %s, i32 2)
  call void @ext(ptr %s, i32 3)
  ret void
}

define internal void @w2(ptr %s) inlinehint optsize {
  call void @ext(ptr %s, i32 4)
  call void @ext(ptr %s, i32 5)
  call void @ext(ptr %s, i32 6)
  ret void
}

define void @oper1(ptr %s) optsize {
  call void @op(ptr %s, ptr @w1)
  ret void
}

define void @oper2(ptr %s) optsize {
  call void @op(ptr %s, ptr @w2)
  ret void
}
