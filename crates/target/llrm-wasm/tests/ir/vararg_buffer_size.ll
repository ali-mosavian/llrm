target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

declare void @take(ptr)
declare i32 @sum(i32, ...)

define i32 @f() {
  %a = alloca [6 x i32], align 4
  call void @take(ptr %a)
  %r = call i32 (i32, ...) @sum(i32 1, i32 5)
  ret i32 %r
}
