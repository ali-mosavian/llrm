target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

define i32 @f(ptr %p, i32 %n) {
entry:
  %v = load i16, ptr %p, align 2
  %s = sext i16 %v to i32
  %z = zext i16 %v to i32
  %c = icmp eq i32 %n, 0
  br i1 %c, label %a, label %b

a:
  ret i32 %s

b:
  ret i32 %z
}
