target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define i32 @exp(i64 %x) {
  %s = lshr i64 %x, 52
  %t = trunc i64 %s to i32
  %e = and i32 %t, 2047
  ret i32 %e
}
define i64 @exp64(i64 %x) {
  %s = lshr i64 %x, 52
  %e = and i64 %s, 2047
  ret i64 %e
}
