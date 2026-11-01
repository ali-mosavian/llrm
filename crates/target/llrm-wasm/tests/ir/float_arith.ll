define double @a(double %x, double %y) {
  %s = fadd double %x, %y
  %d = fsub double %s, 1.5
  %m = fmul double %d, %y
  %q = fdiv double %m, 2.5e3
  %n = fneg double %q
  ret double %n
}
define float @b(float %x, float %y) {
  %s = fadd float %x, %y
  %m = fmul float %s, 0x3FB99999A0000000
  %q = fdiv float %m, %y
  ret float %q
}
define i32 @c(double %x, double %y) {
  %c = fcmp olt double %x, %y
  %r = zext i1 %c to i32
  ret i32 %r
}
define double @d(ptr %p, i32 %i) {
  %q = getelementptr double, ptr %p, i32 %i
  %v = load double, ptr %q, align 8
  store double %v, ptr %p, align 8
  %f = load float, ptr %p, align 4
  %e = fpext float %f to double
  %r = fadd double %v, %e
  ret double %r
}
define float @e(double %x) {
  %t = fptrunc double %x to float
  ret float %t
}
