target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"
define double @half(i32 %w) {
  %d = uitofp i32 %w to double
  %r = fmul double %d, 5.000000e-01
  ret double %r
}
define i64 @sixteen(double %d) {
  %m = fmul double %d, 1.600000e+01
  %r = fptosi double %m to i64
  ret i64 %r
}
define double @quarter(i64 %x) {
  %d = sitofp i64 %x to double
  %r = fmul double 2.500000e-01, %d
  ret double %r
}

define double @not_a_power_of_two(i32 %w) {
  %d = sitofp i32 %w to double
  %r = fmul double %d, 3.000000e+00
  ret double %r
}

define i64 @too_many_bits(double %d) {
  %m = fmul double %d, 0x4200000000000000
  %r = fptosi double %m to i32
  %x = sext i32 %r to i64
  ret i64 %x
}

define double @converted_twice(i32 %w, ptr %p) {
  %d = sitofp i32 %w to double
  %r = fmul double %d, 5.000000e-01
  store double %d, ptr %p, align 8
  ret double %r
}
