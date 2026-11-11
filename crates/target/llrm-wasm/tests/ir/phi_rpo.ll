target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

declare void @g()

; `use` is laid out before `join`, which holds the constant-i1 phi it branches on.
define i32 @f(i1 %a) {
entry:
  br i1 %a, label %m1, label %m2
use:
  br i1 %flag, label %yes, label %no
m1:
  br label %join
m2:
  br label %join
join:
  %flag = phi i1 [ true, %m1 ], [ false, %m2 ]
  call void @g()
  br label %use
yes:
  call void @g()
  ret i32 1
no:
  ret i32 0
}
