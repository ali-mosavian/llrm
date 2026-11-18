target triple = "aarch64-unknown-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
define void @f(ptr %p, ptr %q, ptr %r) {
  %x = load <4 x float>, ptr %p, align 16
  %y = load <4 x i32>, ptr %q, align 16
  %c = fcmp uno <4 x float> %x, zeroinitializer
  %s = select <4 x i1> %c, <4 x i32> <i32 2126512128, i32 2126512128, i32 2126512128, i32 2126512128>, <4 x i32> %y
  store <4 x i32> %s, ptr %r, align 16
  ret void
}
