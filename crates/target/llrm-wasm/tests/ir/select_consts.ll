target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
define i32 @f(ptr %p, i32 %a, i32 %b) {
  %c = icmp sgt i32 %a, %b
  %s = select i1 %c, i32 1, i32 2
  %q = getelementptr [4 x i8], ptr %p, i32 %s
  %v = load i32, ptr %q
  ret i32 %v
}
