target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

declare i32 @by_value([2 x i64], i32)

define i32 @caller(ptr %p) {
  %v = load [2 x i64], ptr %p, align 8
  %r = call i32 @by_value([2 x i64] %v, i32 9)
  ret i32 %r
}
