target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

define i64 @f(ptr noalias %a, ptr noalias %b) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %s = phi i64 [ 0, %entry ], [ %sum, %loop ]
  %pa = getelementptr inbounds i16, ptr %a, i64 %i
  %va = load i16, ptr %pa, align 2
  %xa = sext i16 %va to i64
  %pb = getelementptr inbounds i16, ptr %b, i64 %i
  %vb = load i16, ptr %pb, align 2
  %xb = sext i16 %vb to i64
  %t = mul nsw i64 %xa, %xb
  %sum = add nsw i64 %s, %t
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 2000
  br i1 %done, label %exit, label %loop
exit:
  %r = phi i64 [ %sum, %loop ]
  ret i64 %r
}
