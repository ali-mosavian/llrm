target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
declare i32 @sum(i32, ...)
define i32 @f() {
  %r = tail call i32 (i32, ...) @sum(i32 4, i32 1, i32 2, i32 3, i32 4)
  ret i32 %r
}
