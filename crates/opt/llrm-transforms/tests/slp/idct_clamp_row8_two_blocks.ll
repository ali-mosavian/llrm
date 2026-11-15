target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
declare float @llvm.fmuladd.f32(float, float, float)
define void @f(ptr %in, ptr %coef, ptr %out) {
entry:
  %a = load float, ptr %in, align 4
  %p0 = getelementptr inbounds float, ptr %coef, i64 0
  %c0 = load float, ptr %p0, align 4
  %p1 = getelementptr inbounds float, ptr %coef, i64 1
  %c1 = load float, ptr %p1, align 4
  %p2 = getelementptr inbounds float, ptr %coef, i64 2
  %c2 = load float, ptr %p2, align 4
  %p3 = getelementptr inbounds float, ptr %coef, i64 3
  %c3 = load float, ptr %p3, align 4
  %p4 = getelementptr inbounds float, ptr %coef, i64 4
  %c4 = load float, ptr %p4, align 4
  %p5 = getelementptr inbounds float, ptr %coef, i64 5
  %c5 = load float, ptr %p5, align 4
  %p6 = getelementptr inbounds float, ptr %coef, i64 6
  %c6 = load float, ptr %p6, align 4
  %p7 = getelementptr inbounds float, ptr %coef, i64 7
  %c7 = load float, ptr %p7, align 4
  br label %body
body:
  %m0 = call float @llvm.fmuladd.f32(float %a, float %c0, float 1.0)
  %f0 = fadd float %m0, 1.285000e+02
  %g0 = fcmp ugt float %f0, 0.0
  %i0 = fptosi float %f0 to i32
  %t0 = trunc i32 %i0 to i8
  %s0 = select i1 %g0, i8 %t0, i8 0
  %m1 = call float @llvm.fmuladd.f32(float %a, float %c1, float 1.0)
  %f1 = fadd float %m1, 1.285000e+02
  %g1 = fcmp ugt float %f1, 0.0
  %i1 = fptosi float %f1 to i32
  %t1 = trunc i32 %i1 to i8
  %s1 = select i1 %g1, i8 %t1, i8 0
  %m2 = call float @llvm.fmuladd.f32(float %a, float %c2, float 1.0)
  %f2 = fadd float %m2, 1.285000e+02
  %g2 = fcmp ugt float %f2, 0.0
  %i2 = fptosi float %f2 to i32
  %t2 = trunc i32 %i2 to i8
  %s2 = select i1 %g2, i8 %t2, i8 0
  %m3 = call float @llvm.fmuladd.f32(float %a, float %c3, float 1.0)
  %f3 = fadd float %m3, 1.285000e+02
  %g3 = fcmp ugt float %f3, 0.0
  %i3 = fptosi float %f3 to i32
  %t3 = trunc i32 %i3 to i8
  %s3 = select i1 %g3, i8 %t3, i8 0
  %m4 = call float @llvm.fmuladd.f32(float %a, float %c4, float 1.0)
  %f4 = fadd float %m4, 1.285000e+02
  %g4 = fcmp ugt float %f4, 0.0
  %i4 = fptosi float %f4 to i32
  %t4 = trunc i32 %i4 to i8
  %s4 = select i1 %g4, i8 %t4, i8 0
  %m5 = call float @llvm.fmuladd.f32(float %a, float %c5, float 1.0)
  %f5 = fadd float %m5, 1.285000e+02
  %g5 = fcmp ugt float %f5, 0.0
  %i5 = fptosi float %f5 to i32
  %t5 = trunc i32 %i5 to i8
  %s5 = select i1 %g5, i8 %t5, i8 0
  %m6 = call float @llvm.fmuladd.f32(float %a, float %c6, float 1.0)
  %f6 = fadd float %m6, 1.285000e+02
  %g6 = fcmp ugt float %f6, 0.0
  %i6 = fptosi float %f6 to i32
  %t6 = trunc i32 %i6 to i8
  %s6 = select i1 %g6, i8 %t6, i8 0
  %m7 = call float @llvm.fmuladd.f32(float %a, float %c7, float 1.0)
  %f7 = fadd float %m7, 1.285000e+02
  %g7 = fcmp ugt float %f7, 0.0
  %i7 = fptosi float %f7 to i32
  %t7 = trunc i32 %i7 to i8
  %s7 = select i1 %g7, i8 %t7, i8 0
  %o0 = getelementptr inbounds i8, ptr %out, i64 0
  store i8 %s0, ptr %o0, align 1
  %o1 = getelementptr inbounds i8, ptr %out, i64 1
  store i8 %s1, ptr %o1, align 1
  %o2 = getelementptr inbounds i8, ptr %out, i64 2
  store i8 %s2, ptr %o2, align 1
  %o3 = getelementptr inbounds i8, ptr %out, i64 3
  store i8 %s3, ptr %o3, align 1
  %o4 = getelementptr inbounds i8, ptr %out, i64 4
  store i8 %s4, ptr %o4, align 1
  %o5 = getelementptr inbounds i8, ptr %out, i64 5
  store i8 %s5, ptr %o5, align 1
  %o6 = getelementptr inbounds i8, ptr %out, i64 6
  store i8 %s6, ptr %o6, align 1
  %o7 = getelementptr inbounds i8, ptr %out, i64 7
  store i8 %s7, ptr %o7, align 1
  ret void
}
