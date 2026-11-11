target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
define i32 @f(i32 %x) {
  %m0 = mul i32 %x, 1664525
  %x1 = add i32 %m0, 1013904223
  %a1 = mul i32 %x1, 89373
  %m1 = mul i32 %x1, 1664525
  %x2 = add i32 %m1, 1013904223
  %a2 = mul i32 %x2, 3
  %r = add i32 %a1, %a2
  ret i32 %r
}
