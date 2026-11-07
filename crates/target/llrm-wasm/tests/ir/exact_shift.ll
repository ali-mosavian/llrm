target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
define i32 @f(i32 %x, ptr %p) {
  %h = lshr exact i32 %x, 1
  %a = shl nsw i32 %h, 2
  store i32 %h, ptr %p
  ret i32 %a
}
