target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

declare void @cold(i32)

define void @greater(double %x, double %y) {
  %c = fcmp ogt double %x, %y
  br i1 %c, label %t, label %f
t:
  call void @cold(i32 1)
  ret void
f:
  ret void
}

; `one` is two conditions: mi then gt.
define void @not_equal(float %x, float %y) {
  %c = fcmp one float %x, %y
  br i1 %c, label %t, label %f
t:
  call void @cold(i32 2)
  ret void
f:
  ret void
}
