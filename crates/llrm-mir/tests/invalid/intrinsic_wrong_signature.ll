; invalid: Intrinsic has incorrect argument type!
declare i16 @llvm.smax.i16(i16, i32)

define i16 @f(i16 %a, i32 %b) {
  %r = call i16 @llvm.smax.i16(i16 %a, i32 %b)
  ret i16 %r
}
