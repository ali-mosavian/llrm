target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

@vx = global [6 x i32] zeroinitializer
@vy = global [6 x i32] zeroinitializer

define void @step(i32 %a, i32 %b) {
entry:
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %n, %loop ]
  %px = getelementptr inbounds [6 x i32], ptr @vx, i32 0, i32 %i
  %py = getelementptr inbounds [6 x i32], ptr @vy, i32 0, i32 %i
  %x = load i32, ptr %px
  %y = load i32, ptr %py
  %x1 = add i32 %x, %a
  %y1 = add i32 %y, %b
  %x2 = sdiv i32 %x1, -16
  %y2 = sdiv i32 %y1, -16
  %x3 = add i32 %x2, %x1
  %y3 = add i32 %y2, %y1
  store i32 %x3, ptr %px
  store i32 %y3, ptr %py
  %n = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %n, 6
  br i1 %done, label %exit, label %loop

exit:
  ret void
}
