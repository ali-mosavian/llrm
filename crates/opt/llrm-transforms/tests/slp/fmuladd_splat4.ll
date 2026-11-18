target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
declare float @llvm.fmuladd.f32(float, float, float)
define void @f(ptr %in, ptr %coef, ptr %out) {
  %a = load float, ptr %in, align 4
  %c0 = load float, ptr %coef, align 4
  %p1 = getelementptr inbounds float, ptr %coef, i64 1
  %c1 = load float, ptr %p1, align 4
  %p2 = getelementptr inbounds float, ptr %coef, i64 2
  %c2 = load float, ptr %p2, align 4
  %p3 = getelementptr inbounds float, ptr %coef, i64 3
  %c3 = load float, ptr %p3, align 4
  %m0 = call float @llvm.fmuladd.f32(float %a, float %c0, float 1.0)
  %m1 = call float @llvm.fmuladd.f32(float %a, float %c1, float 1.0)
  %m2 = call float @llvm.fmuladd.f32(float %a, float %c2, float 1.0)
  %m3 = call float @llvm.fmuladd.f32(float %a, float %c3, float 1.0)
  %f0 = fadd float %m0, 1.285000e+02
  %f1 = fadd float %m1, 1.285000e+02
  %f2 = fadd float %m2, 1.285000e+02
  %f3 = fadd float %m3, 1.285000e+02
  store float %f0, ptr %out, align 4
  %o1 = getelementptr inbounds float, ptr %out, i64 1
  store float %f1, ptr %o1, align 4
  %o2 = getelementptr inbounds float, ptr %out, i64 2
  store float %f2, ptr %o2, align 4
  %o3 = getelementptr inbounds float, ptr %out, i64 3
  store float %f3, ptr %o3, align 4
  ret void
}
