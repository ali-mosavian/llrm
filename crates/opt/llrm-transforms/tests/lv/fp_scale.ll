target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

; b[i] = a[i] * 2.0f: consecutive floats, the pin widens it (VF 4); it is not interleaved alone.
define void @f(ptr noalias %a, ptr noalias %b) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %pa = getelementptr inbounds float, ptr %a, i64 %i
  %x = load float, ptr %pa, align 4
  %y = fmul float %x, 2.000000e+00
  %pb = getelementptr inbounds float, ptr %b, i64 %i
  store float %y, ptr %pb, align 4
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 1920
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
