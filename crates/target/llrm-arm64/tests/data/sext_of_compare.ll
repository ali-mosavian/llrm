target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define i32 @f(i32 %a, i32 %b) {
  %t = and i32 %a, %b
  %c = icmp ne i32 %t, 0
  %s = sext i1 %c to i32
  ret i32 %s
}
