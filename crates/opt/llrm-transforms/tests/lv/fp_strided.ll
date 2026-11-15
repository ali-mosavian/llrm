target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

declare float @llvm.fmuladd.f32(float, float, float)

define void @f(ptr noalias %a, ptr noalias %b) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %s = shl nuw nsw i64 %i, 3
  %p0 = getelementptr inbounds i32, ptr %a, i64 %s
  %x0 = load i32, ptr %p0, align 4
  %f0 = sitofp i32 %x0 to float
  %m = call float @llvm.fmuladd.f32(float 0.5, float %f0, float 0.0)
  %t = or disjoint i64 %s, 1
  %p1 = getelementptr inbounds i32, ptr %a, i64 %t
  %x1 = load i32, ptr %p1, align 4
  %f1 = sitofp i32 %x1 to float
  %r = call float @llvm.fmuladd.f32(float 0.25, float %f1, float %m)
  %pb = getelementptr inbounds float, ptr %b, i64 %i
  store float %r, ptr %pb, align 4
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 128
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
