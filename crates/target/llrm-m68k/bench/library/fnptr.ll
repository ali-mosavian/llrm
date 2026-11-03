target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @k(ptr %0, i32 %1) {
  %3 = tail call i32 %0(i32 %1)
  %4 = tail call i32 %0(i32 %3)
  %5 = add i32 %1, 1
  %6 = tail call i32 %0(i32 %5)
  %7 = add i32 %6, %4
  ret i32 %7
}
