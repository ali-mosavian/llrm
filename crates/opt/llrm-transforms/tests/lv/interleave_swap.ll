target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

define void @f(ptr noalias %s, ptr noalias %d) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %j = shl nuw nsw i64 %i, 1
  %k = or disjoint i64 %j, 1
  %s0 = getelementptr inbounds i16, ptr %s, i64 %j
  %s1 = getelementptr inbounds i16, ptr %s, i64 %k
  %v0 = load i16, ptr %s0, align 2
  %v1 = load i16, ptr %s1, align 2
  %d0 = getelementptr inbounds i16, ptr %d, i64 %j
  %d1 = getelementptr inbounds i16, ptr %d, i64 %k
  store i16 %v1, ptr %d0, align 2
  store i16 %v0, ptr %d1, align 2
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 960
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
