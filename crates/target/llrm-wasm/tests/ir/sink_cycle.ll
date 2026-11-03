target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

define void @f(ptr %p, i32 %n) {
entry:
  br label %outer

outer:
  %i = phi i32 [ 0, %entry ], [ %i1, %latch ]
  %i1 = add nuw nsw i32 %i, 1
  %c = icmp ult i32 %i, 3
  br i1 %c, label %inner, label %latch

inner:
  %j = phi i32 [ 0, %outer ], [ %j1, %inner ]
  %q = getelementptr i32, ptr %p, i32 %j
  store i32 %i, ptr %q
  %j1 = add i32 %j, 1
  %d = icmp ult i32 %j1, %n
  br i1 %d, label %inner, label %latch

latch:
  %e = icmp ne i32 %i1, 4
  br i1 %e, label %outer, label %exit

exit:
  ret void
}
