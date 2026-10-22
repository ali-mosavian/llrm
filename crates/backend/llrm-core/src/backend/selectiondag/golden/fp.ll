target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"
@d = global double 0.0
define double @f(double %a, float %b, i32 %i) {
  %x = fpext float %b to double
  %s = fadd double %a, %x
  %m = fmul double %s, 2.5
  %c = fcmp olt double %m, %a
  %r = select i1 %c, double %m, double %a
  store double %r, ptr @d
  %n = fneg double %r
  %t = sitofp i32 %i to double
  %q = fsub double %n, %t
  %z = fptosi double %q to i32
  store i32 %z, ptr @d
  ret double %q
}
define float @g(float %a) {
  ret float %a
}
