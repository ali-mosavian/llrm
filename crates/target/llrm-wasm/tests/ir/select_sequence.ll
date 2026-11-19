target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

define i64 @f(i32 %a, i32 %b) {
  %c1 = icmp ne i32 %a, 0
  %c2 = icmp ne i32 %b, 0
  %c = and i1 %c1, %c2
  %s = select i1 %c, i64 5, i64 0
  ret i64 %s
}
