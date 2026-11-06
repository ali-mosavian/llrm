target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
define i32 @z(ptr %p) {
  %v = load i8, ptr %p
  %f = freeze i8 %v
  %e = zext i8 %f to i32
  ret i32 %e
}
define i32 @s(ptr %p) {
  %v = load i8, ptr %p
  %f = freeze i8 %v
  %e = sext i8 %f to i32
  ret i32 %e
}
