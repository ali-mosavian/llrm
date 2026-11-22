target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define void @h(ptr %p, ptr %q, i32 %x, i32 %y) {
  %a = insertelement <4 x i32> zeroinitializer, i32 %x, i64 0
  store <4 x i32> %a, ptr %p
  %b = insertelement <4 x i32> zeroinitializer, i32 %y, i64 0
  store <4 x i32> %b, ptr %q
  ret void
}
