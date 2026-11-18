target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
%V = type { float, float, float }
define void @kernel(ptr %vel, ptr %dir, float %speed, float %dt) {
  %vx = load float, ptr %vel
  %dx = load float, ptr %dir
  %m = fmul float %vx, %dx
  %add = fsub float %speed, %m
  %pos = fcmp ogt float %add, 0.0
  br i1 %pos, label %go, label %out
go:
  %a = fmul float %speed, %dt
  %b = fmul float %a, 10.0
  %c = fcmp ogt float %b, %add
  %d = select i1 %c, float %add, float %b
  %e = fmul float %d, %dx
  %f = fadd float %vx, %e
  store float %f, ptr %vel
  ret void
out:
  ret void
}
@in = global [4 x float] zeroinitializer
declare void @report(float)
define void @driver() {
  %vel = alloca %V
  %dir = alloca %V
  %p0 = load volatile float, ptr @in
  %p1 = load volatile float, ptr getelementptr (float, ptr @in, i32 1)
  %p2 = load volatile float, ptr getelementptr (float, ptr @in, i32 2)
  %p3 = load volatile float, ptr getelementptr (float, ptr @in, i32 3)
  store float 2.0, ptr %vel
  store float 0.5, ptr %dir
  call void @kernel(ptr %vel, ptr %dir, float 3.0, float %p3)
  %r = load float, ptr %vel
  call void @report(float %r)
  ret void
}
