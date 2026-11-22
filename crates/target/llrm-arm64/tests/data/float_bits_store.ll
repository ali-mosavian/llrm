target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

define void @f(ptr %top, ptr %q) {
  %u = load i32, ptr %top, align 8
  %a = bitcast i32 %u to float
  %c = load i8, ptr %q, align 1
  %d = uitofp i8 %c to float
  %s = fsub float %a, %d
  %b = bitcast float %s to i32
  store i32 %b, ptr %top, align 8
  ret void
}
