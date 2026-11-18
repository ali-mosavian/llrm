target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

define void @f(<4 x float> %v, ptr %k, ptr %out) {
entry:
  %k1 = getelementptr inbounds float, ptr %k, i64 1
  %k2 = getelementptr inbounds float, ptr %k, i64 2
  %k3 = getelementptr inbounds float, ptr %k, i64 3
  %o1 = getelementptr inbounds float, ptr %out, i64 1
  %o2 = getelementptr inbounds float, ptr %out, i64 2
  %o3 = getelementptr inbounds float, ptr %out, i64 3
  %e0 = extractelement <4 x float> %v, i64 0
  %e1 = extractelement <4 x float> %v, i64 1
  %e2 = extractelement <4 x float> %v, i64 2
  %e3 = extractelement <4 x float> %v, i64 3
  %c0 = load float, ptr %k, align 4
  %c1 = load float, ptr %k1, align 4
  %c2 = load float, ptr %k2, align 4
  %c3 = load float, ptr %k3, align 4
  %m0 = fmul float %e0, %c0
  %m1 = fmul float %e1, %c1
  %m2 = fmul float %e2, %c2
  %m3 = fmul float %e3, %c3
  store float %m0, ptr %out, align 4
  store float %m1, ptr %o1, align 4
  store float %m2, ptr %o2, align 4
  store float %m3, ptr %o3, align 4
  ret void
}
