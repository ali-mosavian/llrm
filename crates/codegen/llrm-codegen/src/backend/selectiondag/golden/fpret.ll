target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

declare double @g(double)
declare float @h(i32)
declare void @sink(float)

define double @call_double(double %x) {
  %r = call double @g(double %x)
  %s = fadd double %r, %x
  ret double %s
}

define float @call_float(i32 %n) {
  %r = call float @h(i32 %n)
  call void @sink(float %r)
  ret float %r
}
