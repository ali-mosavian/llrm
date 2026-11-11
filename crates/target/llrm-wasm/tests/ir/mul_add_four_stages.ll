target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
define i32 @f(i32 %x, i32 %t) {
  %m0 = mul i32 %x, 1664525
  %x1 = add i32 %m0, 1013904223
  %a1 = mul i32 %x1, 89373
  %b1 = mul i32 %t, 923521
  %t1 = add i32 %a1, %b1
  %m1 = mul i32 %x1, 1664525
  %x2 = add i32 %m1, 1013904223
  %a2 = mul i32 %x2, 2883
  %t2 = add i32 %a2, %t1
  %m2 = mul i32 %x2, 1664525
  %x3 = add i32 %m2, 1013904223
  %a3 = mul i32 %x3, 93
  %t3 = add i32 %a3, %t2
  %m3 = mul i32 %x3, 1664525
  %x4 = add i32 %m3, 1013904223
  %a4 = mul i32 %x4, 3
  %t4 = add i32 %a4, %t3
  ret i32 %t4
}
