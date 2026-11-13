target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

define void @f(ptr noalias %a, ptr noalias %b, ptr noalias %c) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %pa = getelementptr inbounds double, ptr %a, i64 %i
  %x = load double, ptr %pa, align 8
  %pb = getelementptr inbounds double, ptr %b, i64 %i
  %y = load double, ptr %pb, align 8
  %d = fsub double %x, %y
  %r = fneg double %d
  %pc = getelementptr inbounds double, ptr %c, i64 %i
  store double %r, ptr %pc, align 8
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 1024
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
