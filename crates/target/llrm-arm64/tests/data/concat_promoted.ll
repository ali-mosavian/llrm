target triple = "aarch64-unknown-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
define void @f(ptr %p, ptr %q, ptr %o) {
  %a = load <4 x i8>, ptr %p, align 4
  %b = load <4 x i8>, ptr %q, align 4
  %c = shufflevector <4 x i8> %a, <4 x i8> %b, <8 x i32> <i32 0, i32 1, i32 2, i32 3, i32 4, i32 5, i32 6, i32 7>
  store <8 x i8> %c, ptr %o, align 8
  ret void
}
