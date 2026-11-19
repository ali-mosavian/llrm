target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @k(i32 %0, i32 %1) {
  %3 = mul i32 %1, %0
  %4 = sdiv i32 %0, %1
  %5 = add i32 %4, %3
  %6 = mul i32 %4, %1
  %7 = sub i32 %0, %6
  %8 = add i32 %5, %7
  ret i32 %8
}
