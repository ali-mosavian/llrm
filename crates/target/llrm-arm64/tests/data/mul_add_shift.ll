target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define i32 @f(i32 %n, i32 %x) {
  %m = mul i32 %x, 7
  %a = add i32 %m, %n
  ret i32 %a
}
