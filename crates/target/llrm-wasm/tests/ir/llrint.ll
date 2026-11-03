declare i32 @llvm.lrint.i32.f64(double)
declare i32 @llvm.lrint.i32.f32(float)
declare i64 @llvm.llrint.i64.f64(double)
declare i64 @llvm.llrint.i64.f32(float)
declare i32 @llvm.llrint.i32.f64(double)

define i32 @lrint32(double %x) {
  %r = call i32 @llvm.lrint.i32.f64(double %x)
  ret i32 %r
}

define i32 @lrint32f(float %x) {
  %r = call i32 @llvm.lrint.i32.f32(float %x)
  ret i32 %r
}

define i64 @llrint64(double %x) {
  %r = call i64 @llvm.llrint.i64.f64(double %x)
  ret i64 %r
}

define i64 @llrint64f(float %x) {
  %r = call i64 @llvm.llrint.i64.f32(float %x)
  ret i64 %r
}

define i16 @llrint16(double %x) {
  %r = call i32 @llvm.llrint.i32.f64(double %x)
  %t = trunc i32 %r to i16
  ret i16 %t
}
