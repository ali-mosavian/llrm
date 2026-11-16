target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
; b[i] = a[i] > 100 ? 100 : a[i] (i32, icmp + select, 1024 trips)
define void @f(ptr noalias %a, ptr noalias %b) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %pa = getelementptr inbounds i32, ptr %a, i64 %i
  %x = load i32, ptr %pa, align 4
  %c = icmp sgt i32 %x, 100
  %s = select i1 %c, i32 100, i32 %x
  %pb = getelementptr inbounds i32, ptr %b, i64 %i
  store i32 %s, ptr %pb, align 4
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 1024
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
