target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
declare i16 @llvm.smul.fix.sat.i16(i16, i16, i32)
define i16 @f(i16 %a, i16 %b) {
  %r = call i16 @llvm.smul.fix.sat.i16(i16 %a, i16 %b, i32 7)
  ret i16 %r
}
