target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"
declare void @sink(i32, i32, i32)
define void @f(i8 %0, i8 %1) {
  %3 = zext i8 %1 to i32
  %4 = zext i8 %0 to i32
  %5 = mul i32 %3, %4
  %6 = sub i32 %4, %3
  %7 = mul i32 %6, %4
  tail call void @sink(i32 %5, i32 0, i32 %7)
  ret void
}
