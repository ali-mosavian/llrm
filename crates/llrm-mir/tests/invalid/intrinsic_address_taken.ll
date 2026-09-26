; invalid: Cannot take the address of an intrinsic!
declare i16 @llvm.smax.i16(i16, i16)

define ptr @f() {
  ret ptr @llvm.smax.i16
}
