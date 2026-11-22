target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

define i1 @f(i8 %c) {
  %u = uitofp i8 %c to float
  %b = bitcast float %u to i32
  %t = icmp slt i32 %b, 0
  ret i1 %t
}
