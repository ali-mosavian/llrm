target triple = "aarch64-unknown-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
define void @f(ptr %p, ptr %o) {
  %w = load <16 x i8>, ptr %p, align 16
  %e = shufflevector <16 x i8> %w, <16 x i8> poison, <2 x i32> <i32 2, i32 3>
  %x = shufflevector <2 x i8> %e, <2 x i8> poison, <16 x i32> <i32 poison, i32 poison, i32 poison, i32 poison, i32 poison, i32 0, i32 1, i32 poison, i32 poison, i32 poison, i32 poison, i32 poison, i32 poison, i32 poison, i32 poison, i32 poison>
  store <16 x i8> %x, ptr %o, align 16
  ret void
}
