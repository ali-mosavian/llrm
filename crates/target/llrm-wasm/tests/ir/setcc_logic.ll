target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

define i32 @f(i32 %a, i32 %z) {
  %c1 = icmp ne i32 %a, 7
  %c2 = icmp ne i32 %z, 9
  %o = or i1 %c1, %c2
  %x = zext i1 %o to i32
  ret i32 %x
}
