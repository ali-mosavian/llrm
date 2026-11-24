target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

define internal i32 @mix(i32 %a, i32 %b) optsize {
  %1 = mul i32 %a, 7
  %2 = xor i32 %1, %b
  %3 = shl i32 %2, 3
  %4 = add i32 %3, %a
  %5 = mul i32 %4, 11
  %6 = lshr i32 %5, 2
  %7 = xor i32 %6, %b
  %8 = add i32 %7, %5
  %9 = mul i32 %8, 13
  %10 = sub i32 %9, %a
  ret i32 %10
}

define i32 @f(i32 %x, i32 %y) optsize {
  %1 = call i32 @mix(i32 %x, i32 %y)
  %2 = call i32 @mix(i32 %y, i32 %1)
  %3 = call i32 @mix(i32 %2, i32 %x)
  %4 = add i32 %1, %3
  ret i32 %4
}
