define double @s16(i16 %x) {
  %r = sitofp i16 %x to double
  ret double %r
}
define float @u8(i8 %x) {
  %r = uitofp i8 %x to float
  ret float %r
}
define double @s8(i8 %x) {
  %r = sitofp i8 %x to double
  ret double %r
}
define i16 @f16(double %x) {
  %r = fptosi double %x to i16
  ret i16 %r
}
define i8 @fu8(float %x) {
  %r = fptoui float %x to i8
  ret i8 %r
}
