target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

define i32 @f(i32 %x) {
  %u = add i32 %x, 3
  %a = mul i32 %u, 5
  %b = mul i32 %x, 5
  %s = add i32 %a, %b
  ret i32 %s
}
