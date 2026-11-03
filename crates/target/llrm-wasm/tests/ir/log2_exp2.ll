declare double @llvm.log2.f64(double)
declare float @llvm.log2.f32(float)
declare double @llvm.exp2.f64(double)
declare float @llvm.exp2.f32(float)

define double @log2d(double %x) {
  %r = call double @llvm.log2.f64(double %x)
  ret double %r
}

define float @log2f_(float %x) {
  %r = call float @llvm.log2.f32(float %x)
  ret float %r
}

define double @exp2d(double %x) {
  %r = call double @llvm.exp2.f64(double %x)
  ret double %r
}

define float @exp2f_(float %x) {
  %r = call float @llvm.exp2.f32(float %x)
  ret float %r
}
