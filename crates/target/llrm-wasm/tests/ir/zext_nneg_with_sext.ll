target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

define i32 @f(ptr %p) {
  %v = load i16, ptr %p
  %s = sext i16 %v to i32
  %z = zext nneg i16 %v to i32
  %r = add i32 %s, %z
  ret i32 %r
}
