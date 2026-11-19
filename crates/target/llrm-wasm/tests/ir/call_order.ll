target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

declare i32 @g()
declare i32 @take(ptr, i32)

define i32 @h(ptr %p) {
  %r = call i32 @g()
  store i32 1, ptr %p
  %s = add i32 %r, 1
  ret i32 %s
}

define i32 @frame(i32 %n) {
  %a = alloca [12 x i32], align 16
  %r = call i32 @take(ptr %a, i32 %n)
  ret i32 %r
}
