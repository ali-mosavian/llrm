target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define i64 @f(ptr %p) {
  %x = load i16, ptr %p
  %d = sdiv i16 %x, 4
  %s = sext i16 %d to i64
  ret i64 %s
}
