target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

; jpeg_fixed's convert: three byte stores at 3 * i, 3 * i + 1, 3 * i + 2.
define void @f(ptr noalias %d, ptr noalias %s) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %ps = getelementptr inbounds i8, ptr %s, i64 %i
  %v = load i8, ptr %ps, align 1
  %w = add i8 %v, 1
  %x = add i8 %v, 2
  %j = mul nuw nsw i64 %i, 3
  %a = getelementptr inbounds i8, ptr %d, i64 %j
  store i8 %v, ptr %a, align 1
  %k = add nuw nsw i64 %j, 1
  %b = getelementptr inbounds i8, ptr %d, i64 %k
  store i8 %w, ptr %b, align 1
  %l = add nuw nsw i64 %j, 2
  %c = getelementptr inbounds i8, ptr %d, i64 %l
  store i8 %x, ptr %c, align 1
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 960
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
