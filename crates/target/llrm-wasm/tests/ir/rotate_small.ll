target triple = "wasm32-unknown-unknown"

declare i8 @llvm.fshl.i8(i8, i8, i8)
declare i8 @llvm.fshr.i8(i8, i8, i8)
declare i16 @llvm.fshl.i16(i16, i16, i16)

define i8 @rotl8_var(i8 %x, i8 %n) {
  %r = call i8 @llvm.fshl.i8(i8 %x, i8 %x, i8 %n)
  ret i8 %r
}

define i8 @rotr8_var(i8 %x, i8 %n) {
  %r = call i8 @llvm.fshr.i8(i8 %x, i8 %x, i8 %n)
  ret i8 %r
}

define i8 @rotl8_const(i8 %x) {
  %r = call i8 @llvm.fshl.i8(i8 %x, i8 %x, i8 3)
  ret i8 %r
}

define i16 @rotl16_var(i16 %x, i16 %n) {
  %r = call i16 @llvm.fshl.i16(i16 %x, i16 %x, i16 %n)
  ret i16 %r
}
