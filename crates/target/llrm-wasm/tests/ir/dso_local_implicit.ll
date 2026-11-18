target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

@preemptible = external global [64 x i8]
@hidden = external hidden global [64 x i8]

define void @f(i32 %i) {
  %a = getelementptr i8, ptr @preemptible, i32 %i
  %a8 = getelementptr i8, ptr %a, i32 8
  store i8 1, ptr %a8
  ret void
}

define void @g(i32 %i) {
  %a = getelementptr i8, ptr @hidden, i32 %i
  %a8 = getelementptr i8, ptr %a, i32 8
  store i8 1, ptr %a8
  ret void
}
