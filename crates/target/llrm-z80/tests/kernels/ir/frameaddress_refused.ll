target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
declare ptr @llvm.frameaddress.p0(i32)
define ptr @f() {
  %p = call ptr @llvm.frameaddress.p0(i32 0)
  ret ptr %p
}
