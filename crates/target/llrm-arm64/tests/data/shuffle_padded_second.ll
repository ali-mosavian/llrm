target triple = "aarch64-unknown-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
define void @f(ptr %p, ptr %q, ptr %o) {
  %x = load <8 x i8>, ptr %p, align 8
  %y = load <8 x i8>, ptr %q, align 8
  %t = shufflevector <8 x i8> %x, <8 x i8> %y, <16 x i32> <i32 0, i32 8, i32 1, i32 9, i32 2, i32 10, i32 3, i32 11, i32 4, i32 12, i32 5, i32 13, i32 6, i32 14, i32 7, i32 15>
  store <16 x i8> %t, ptr %o, align 16
  ret void
}
