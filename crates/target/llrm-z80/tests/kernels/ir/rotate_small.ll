target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"

declare i8 @llvm.fshl.i8(i8, i8, i8)
declare i16 @llvm.fshr.i16(i16, i16, i16)

define i8 @rotl8(i8 %x, i8 %n) {
  %r = call i8 @llvm.fshl.i8(i8 %x, i8 %x, i8 %n)
  ret i8 %r
}

define i16 @rotr16(i16 %x, i16 %n) {
  %r = call i16 @llvm.fshr.i16(i16 %x, i16 %x, i16 %n)
  ret i16 %r
}

define i16 @rotr16_by_nine(i16 %x) {
  %r = call i16 @llvm.fshr.i16(i16 %x, i16 %x, i16 9)
  ret i16 %r
}
