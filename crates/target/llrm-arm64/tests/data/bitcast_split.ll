target triple = "aarch64-unknown-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
define void @f(ptr %p, ptr %o) {
  %b = load <64 x i8>, ptr %p, align 16
  %a = add <64 x i8> %b, %b
  %c = bitcast <64 x i8> %a to <16 x i32>
  store <16 x i32> %c, ptr %o, align 16
  ret void
}
