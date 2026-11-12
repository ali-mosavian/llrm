target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
define i32 @f(ptr %p) {
  %x = load volatile i16, ptr %p
  %r = srem i16 -1, %x
  %c = icmp eq i16 %r, -1
  %z = zext i1 %c to i32
  ret i32 %z
}
