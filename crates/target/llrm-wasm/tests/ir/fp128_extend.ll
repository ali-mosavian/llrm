target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

define fp128 @widen(double %x) {
  %r = fpext double %x to fp128
  ret fp128 %r
}

define fp128 @widen_float(float %x) {
  %r = fpext float %x to fp128
  ret fp128 %r
}

define double @narrow(fp128 %x) {
  %r = fptrunc fp128 %x to double
  ret double %r
}
