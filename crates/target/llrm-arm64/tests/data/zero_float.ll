target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

define double @zero_double(i1 %c, double %x) {
  %r = select i1 %c, double %x, double 0.0
  ret double %r
}

define float @zero_float(i1 %c, float %x) {
  %r = select i1 %c, float %x, float 0.0
  ret float %r
}
