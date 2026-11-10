target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

define void @copy3(ptr noalias %d, ptr noalias %s) {
  %a = load i8, ptr %s
  store i8 %a, ptr %d
  %s1 = getelementptr i8, ptr %s, i32 1
  %d1 = getelementptr i8, ptr %d, i32 1
  %b = load i8, ptr %s1
  store i8 %b, ptr %d1
  %s2 = getelementptr i8, ptr %s, i32 2
  %d2 = getelementptr i8, ptr %d, i32 2
  %c = load i8, ptr %s2
  store i8 %c, ptr %d2
  ret void
}
