target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

; sfm16_float_add: two floats per 44-byte row, an fadd, a NaN canonicalized by fcmp uno and select, a sum.
define i32 @f(ptr noalias %a) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %acc = phi i32 [ 0, %entry ], [ %sum, %loop ]
  %row = getelementptr inbounds [11 x i32], ptr %a, i64 %i
  %pa = getelementptr inbounds i32, ptr %row, i64 0
  %pb = getelementptr inbounds i32, ptr %row, i64 1
  %x = load float, ptr %pa, align 4
  %y = load float, ptr %pb, align 4
  %s = fadd float %x, %y
  %nan = fcmp uno float %s, 0.000000e+00
  %bits = bitcast float %s to i32
  %c = select i1 %nan, i32 2143289344, i32 %bits
  %sum = add i32 %c, %acc
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 400
  br i1 %done, label %exit, label %loop
exit:
  %r = phi i32 [ %sum, %loop ]
  ret i32 %r
}
