target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

; A float in a range: `fcmp; fccmp; csel`.
define i32 @in_range(float %x, i32 %a, i32 %b) {
  %hi = fcmp olt float %x, 2.0e9
  %lo = fcmp oge float %x, -2.0e9
  %both = and i1 %hi, %lo
  %r = select i1 %both, i32 %a, i32 %b
  ret i32 %r
}

; `one` is two flag tests: `ord` and `une`.
define i32 @different(double %x, double %y, double %z, i32 %a, i32 %b) {
  %ne = fcmp one double %x, %y
  %lt = fcmp olt double %x, %z
  %both = and i1 %ne, %lt
  %r = select i1 %both, i32 %a, i32 %b
  ret i32 %r
}

; An or of compares, the chain negated.
define i32 @either(float %x, float %y, i32 %a, i32 %b) {
  %u = fcmp uno float %x, 0.0
  %g = fcmp ogt float %x, %y
  %any = or i1 %u, %g
  %r = select i1 %any, i32 %a, i32 %b
  ret i32 %r
}
