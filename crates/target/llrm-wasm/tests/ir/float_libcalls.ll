define double @libcall(double %a, double %b) {
  %r = frem double %a, %b
  ret double %r
}
define double @sc(double %a) {
  %s = call double @llvm.sin.f64(double %a)
  %c = call double @llvm.cos.f64(double %a)
  %t = call double @llvm.atan.f64(double %a)
  %x = fadd double %s, %c
  %y = fadd double %x, %t
  ret double %y
}
define float @sf(float %a) {
  %s = call float @llvm.sin.f32(float %a)
  ret float %s
}
declare double @llvm.sin.f64(double)
declare double @llvm.cos.f64(double)
declare double @llvm.atan.f64(double)
declare float @llvm.sin.f32(float)
define i32 @lr32(double %a) {
  %r = call i32 @llvm.lrint.i32.f64(double %a)
  ret i32 %r
}
define i16 @lr16(double %a) {
  %r = call i16 @llvm.lrint.i16.f64(double %a)
  ret i16 %r
}
define i32 @lrf(float %a) {
  %r = call i32 @llvm.lrint.i32.f32(float %a)
  ret i32 %r
}
define double @rn(double %a) {
  %r = call double @llvm.rint.f64(double %a)
  ret double %r
}
define double @ab(double %a) {
  %r = call double @llvm.fabs.f64(double %a)
  ret double %r
}
declare i32 @llvm.lrint.i32.f64(double)
declare i16 @llvm.lrint.i16.f64(double)
declare i32 @llvm.lrint.i32.f32(float)
declare double @llvm.rint.f64(double)
declare double @llvm.fabs.f64(double)
