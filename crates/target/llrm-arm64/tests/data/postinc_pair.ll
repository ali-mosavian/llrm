target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@v = dso_local global [400 x [11 x i32]] zeroinitializer, align 4

define float @sum() {
entry:
  br label %loop

loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %acc = phi float [ 0.000000e+00, %entry ], [ %sum, %loop ]
  %p0 = getelementptr [400 x [11 x i32]], ptr @v, i64 0, i64 %i, i64 0
  %p1 = getelementptr [400 x [11 x i32]], ptr @v, i64 0, i64 %i, i64 1
  %a = load float, ptr %p0, align 4
  %b = load float, ptr %p1, align 4
  %m = fmul float %a, %b
  %sum = fadd float %acc, %m
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 400
  br i1 %done, label %exit, label %loop

exit:
  ret float %sum
}
