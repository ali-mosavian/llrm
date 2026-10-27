target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@d = global double 0.0, align 8
@f = global float 0.0, align 4

define void @set() {
  store double 3.141592653589793, ptr @d, align 8
  store float 0x3FB99999A0000000, ptr @f, align 4
  store volatile double 1.0e300, ptr @d, align 8
  ret void
}

define void @clear() {
  store double 0.0, ptr @d, align 8
  ret void
}
