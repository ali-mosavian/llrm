target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
; c[i] = a[i] < b[i] ? a[i] : b[i] (float fcmp olt + select)
define void @f(ptr noalias %a, ptr noalias %b, ptr noalias %c) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %pa = getelementptr inbounds float, ptr %a, i64 %i
  %x = load float, ptr %pa, align 4
  %pb = getelementptr inbounds float, ptr %b, i64 %i
  %y = load float, ptr %pb, align 4
  %lt = fcmp olt float %x, %y
  %m = select i1 %lt, float %x, float %y
  %pc = getelementptr inbounds float, ptr %c, i64 %i
  store float %m, ptr %pc, align 4
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 1024
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
