target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

define float @scale(ptr %p) {
  %v = load i16, ptr %p, align 2
  %f = sitofp i16 %v to float
  %m = fmul float %f, 2.500000e-01
  ret float %m
}
