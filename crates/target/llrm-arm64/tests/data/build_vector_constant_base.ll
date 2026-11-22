target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define void @f(ptr %p, i32 %x) {
  %v = insertelement <4 x i32> zeroinitializer, i32 %x, i64 0
  store <4 x i32> %v, ptr %p
  ret void
}
define void @g(ptr %p, i32 %x, i32 %y) {
  %a = insertelement <4 x i32> <i32 7, i32 7, i32 7, i32 7>, i32 %x, i64 1
  %v = insertelement <4 x i32> %a, i32 %y, i64 3
  store <4 x i32> %v, ptr %p
  ret void
}
