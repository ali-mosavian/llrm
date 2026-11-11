target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define i32 @f(i32 %x) {
  %s = lshr i32 %x, 29
  %a = and i32 %s, 3
  ret i32 %a
}
define i32 @g(i32 %x, i32 %y) {
  %s = lshr i32 %x, 29
  %a = and i32 %s, 3
  %r = add i32 %y, %a
  ret i32 %r
}
