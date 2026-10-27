target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"
@d = global double 0.0
@s = global float 0.0
define double @arith(double %a, double %b) {
  %s = fadd double %a, %b
  %d = fsub double %s, %b
  %m = fmul double %d, %a
  %q = fdiv double %m, %b
  ret double %q
}
define double @loadop(ptr %p, double %a) {
  %v = load double, ptr %p
  %s = fadd double %a, %v
  ret double %s
}
define void @storeop(double %a) {
  store double %a, ptr @d
  ret void
}
define double @widen(float %f) {
  %x = fpext float %f to double
  ret double %x
}
define float @narrow(double %a) {
  %x = fptrunc double %a to float
  ret float %x
}
define double @from_int(i32 %i) {
  %x = sitofp i32 %i to double
  ret double %x
}
define i32 @to_int(double %a) {
  %x = fptosi double %a to i32
  ret i32 %x
}
define double @constants() {
  %a = fadd double 0.0, 1.0
  ret double %a
}
define double @negate(double %a) {
  %x = fneg double %a
  ret double %x
}
define i32 @compare(double %a, double %b) {
  %c = fcmp olt double %a, %b
  br i1 %c, label %t, label %e
t:
  ret i32 1
e:
  ret i32 2
}
