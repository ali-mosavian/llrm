declare double @llvm.fabs.f64(double)
declare double @llvm.sqrt.f64(double)
declare float @llvm.sqrt.f32(float)
declare double @llvm.copysign.f64(double, double)
declare double @llvm.floor.f64(double)
declare double @llvm.ceil.f64(double)
declare double @llvm.trunc.f64(double)
declare float @llvm.fabs.f32(float)
declare double @sin(double)
declare float @takef(float, double)
define double @conv1(i32 %i, i64 %l, i32 %u) {
  %a = sitofp i32 %i to double
  %b = sitofp i64 %l to double
  %c = uitofp i32 %u to double
  %s = fadd double %a, %b
  %t = fadd double %s, %c
  ret double %t
}
define i32 @conv2(double %d, float %f) {
  %a = fptosi double %d to i32
  %b = fptoui float %f to i32
  %c = add i32 %a, %b
  ret i32 %c
}
define i64 @conv3(double %d) {
  %a = fptosi double %d to i64
  %b = fptoui double %d to i64
  %c = add i64 %a, %b
  ret i64 %c
}
define float @conv4(i32 %i, i64 %l) {
  %a = sitofp i32 %i to float
  %b = uitofp i64 %l to float
  %c = fadd float %a, %b
  ret float %c
}
define i32 @bits(float %f, i32 %i) {
  %a = bitcast float %f to i32
  %b = bitcast i32 %i to float
  %c = fptosi float %b to i32
  %d = add i32 %a, %c
  ret i32 %d
}
define i32 @cmps(double %x, double %y) {
  %c1 = fcmp oeq double %x, %y
  %c2 = fcmp one double %x, %y
  %c3 = fcmp ogt double %x, %y
  %c4 = fcmp oge double %x, %y
  %c5 = fcmp ole double %x, %y
  %c6 = fcmp une double %x, %y
  %c7 = fcmp ord double %x, %y
  %c8 = fcmp uno double %x, %y
  %c9 = fcmp ult double %x, %y
  %c10 = fcmp ueq double %x, %y
  %z1 = zext i1 %c1 to i32
  %z2 = zext i1 %c2 to i32
  %z3 = zext i1 %c3 to i32
  %z4 = zext i1 %c4 to i32
  %z5 = zext i1 %c5 to i32
  %z6 = zext i1 %c6 to i32
  %z7 = zext i1 %c7 to i32
  %z8 = zext i1 %c8 to i32
  %z9 = zext i1 %c9 to i32
  %z10 = zext i1 %c10 to i32
  %s1 = add i32 %z1, %z2
  %s2 = add i32 %s1, %z3
  %s3 = add i32 %s2, %z4
  %s4 = add i32 %s3, %z5
  %s5 = add i32 %s4, %z6
  %s6 = add i32 %s5, %z7
  %s7 = add i32 %s6, %z8
  %s8 = add i32 %s7, %z9
  %s9 = add i32 %s8, %z10
  ret i32 %s9
}
define double @sel(double %x, double %y, i32 %k) {
  %c = fcmp olt double %x, %y
  %r = select i1 %c, double %x, double %y
  %c2 = icmp eq i32 %k, 0
  %r2 = select i1 %c2, double %r, double 2.0
  ret double %r2
}
define double @branch(double %x, double %y) {
entry:
  %c = fcmp ogt double %x, %y
  br i1 %c, label %a, label %b
a:
  %s = fsub double %x, %y
  ret double %s
b:
  %t = fsub double %y, %x
  ret double %t
}
define float @intrinf(float %x) {
  %a = call float @llvm.fabs.f32(float %x)
  %b = call float @llvm.sqrt.f32(float %a)
  ret float %b
}
define double @libcall(double %x) {
  %a = call double @sin(double %x)
  %b = frem double %a, %x
  ret double %b
}
define float @callf(float %x, double %y) {
  %a = call float @takef(float %x, double %y)
  ret float %a
}
define double @consts() {
  %a = fadd double 0.0, 1.0
  %b = fadd double -0.0, 0x7FF0000000000000
  %c = fadd double %a, %b
  %e = fadd double %c, 4.9e-324
  %f = fadd double %e, 0x3FF0000000000001
  ret double %f
}
