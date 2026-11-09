target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
declare i1 @llvm.is.constant.i16(i16)
define i16 @f(i16 %x) {
  %c = call i1 @llvm.is.constant.i16(i16 %x)
  %r = select i1 %c, i16 1, i16 2
  ret i16 %r
}
