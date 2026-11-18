target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

define void @f(ptr noalias %p, i8 %x, i8 %y) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %j = shl nuw nsw i64 %i, 1
  %a = getelementptr inbounds i8, ptr %p, i64 %j
  store i8 %x, ptr %a, align 1
  %k = or disjoint i64 %j, 1
  %b = getelementptr inbounds i8, ptr %p, i64 %k
  store i8 %y, ptr %b, align 1
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 1000
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
