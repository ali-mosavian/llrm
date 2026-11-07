target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

define i32 @f(ptr %a) nounwind {
entry:
  br label %loop
loop:
  %i = phi i32 [ 1, %entry ], [ %next, %cont ]
  %im = add nsw i32 %i, -1
  %pa = getelementptr i32, ptr %a, i32 %im
  %pb = getelementptr i32, ptr %a, i32 %i
  %x = load i32, ptr %pa
  %y = load i32, ptr %pb
  %bad = icmp sgt i32 %x, %y
  br i1 %bad, label %fail, label %cont
cont:
  %next = add nuw nsw i32 %i, 1
  %d = icmp eq i32 %next, 1024
  br i1 %d, label %ok, label %loop
fail:
  ret i32 -1
ok:
  ret i32 0
}
