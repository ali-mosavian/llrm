target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define double @dadd(double %a, double %b) { %r = fadd double %a, %b ret double %r }
define double @dsub(double %a, double %b) { %r = fsub double %a, %b ret double %r }
define double @dmul(double %a, double %b) { %r = fmul double %a, %b ret double %r }
define double @ddiv(double %a, double %b) { %r = fdiv double %a, %b ret double %r }
define float @fadd2(float %a, float %b) { %r = fadd float %a, %b ret float %r }
define float @fmul2(float %a, float %b) { %r = fmul float %a, %b ret float %r }
define double @ext(float %a) { %r = fpext float %a to double ret double %r }
define float @trunc2(double %a) { %r = fptrunc double %a to float ret float %r }
define double @i2d(i32 %a) { %r = sitofp i32 %a to double ret double %r }
define float @i2f(i32 %a) { %r = sitofp i32 %a to float ret float %r }
define i32 @d2i(double %a) { %r = fptosi double %a to i32 ret i32 %r }
define i32 @flt(double %a, double %b) { %c = fcmp olt double %a, %b %r = zext i1 %c to i32 ret i32 %r }
define i32 @feq(double %a, double %b) { %c = fcmp oeq double %a, %b %r = zext i1 %c to i32 ret i32 %r }
define i32 @fge(float %a, float %b) { %c = fcmp oge float %a, %b %r = zext i1 %c to i32 ret i32 %r }
define double @neg(double %a) { %r = fneg double %a ret double %r }
define double @sel(double %a, double %b, i32 %k) { %c = icmp eq i32 %k, 0 %r = select i1 %c, double %a, double %b ret double %r }
define double @dsum(ptr %p, i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [0, %entry], [%i1, %loop]
  %acc = phi double [0.0, %entry], [%acc1, %loop]
  %q = getelementptr double, ptr %p, i32 %i
  %v = load double, ptr %q
  %acc1 = fadd double %acc, %v
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, %n
  br i1 %c, label %loop, label %done
done:
  ret double %acc1
}
define void @st(ptr %p, double %v) { store double %v, ptr %p ret void }
define double @mix(double %a, float %b, i32 %c) {
  %e = fpext float %b to double
  %m = fmul double %a, %e
  %f = sitofp i32 %c to double
  %r = fadd double %m, %f
  ret double %r
}
