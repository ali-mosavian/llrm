target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

@p = external global i8

define i8 @f() {
  %a = load volatile i8, ptr @p
  %b = load volatile i8, ptr @p
  %c = load volatile i8, ptr @p
  %d = load volatile i8, ptr @p
  %e = load volatile i8, ptr @p
  %f = load volatile i8, ptr @p
  %g = load volatile i8, ptr @p
  %h = load volatile i8, ptr @p
  %i = load volatile i8, ptr @p
  %j = load volatile i8, ptr @p
  %k = load volatile i8, ptr @p
  %l = load volatile i8, ptr @p
  %x1 = xor i8 %l, %k
  %x2 = xor i8 %x1, %j
  %x3 = xor i8 %x2, %i
  %x4 = xor i8 %x3, %h
  %x5 = xor i8 %x4, %g
  %x6 = xor i8 %x5, %f
  %x7 = xor i8 %x6, %e
  %x8 = xor i8 %x7, %d
  %x9 = xor i8 %x8, %c
  %x10 = xor i8 %x9, %b
  %x11 = xor i8 %x10, %a
  ret i8 %x11
}
