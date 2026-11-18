target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

define void @f(ptr %x, ptr %out) {
entry:
  %x1 = getelementptr inbounds float, ptr %x, i64 1
  %x2 = getelementptr inbounds float, ptr %x, i64 2
  %x3 = getelementptr inbounds float, ptr %x, i64 3
  %o1 = getelementptr inbounds float, ptr %out, i64 1
  %o2 = getelementptr inbounds float, ptr %out, i64 2
  %o3 = getelementptr inbounds float, ptr %out, i64 3
  %v0 = load float, ptr %x, align 4
  %v1 = load float, ptr %x1, align 4
  %v2 = load float, ptr %x2, align 4
  %v3 = load float, ptr %x3, align 4
  %m0 = fneg float %v0
  %m1 = fneg float %v1
  %m2 = fneg float %v2
  %m3 = fneg float %v3
  store float %m0, ptr %out, align 4
  store float %m1, ptr %o1, align 4
  store float %m2, ptr %o2, align 4
  store float %m3, ptr %o3, align 4
  ret void
}
