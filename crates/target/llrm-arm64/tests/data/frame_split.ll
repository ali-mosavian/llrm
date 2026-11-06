target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

declare void @fill(ptr)
declare i32 @tally(ptr, i32)

define i32 @buffer_frame(i32 %n) {
  %buf = alloca [1024 x i32], align 4
  call void @fill(ptr %buf)
  %r = call i32 @tally(ptr %buf, i32 %n)
  %s = add i32 %r, %n
  ret i32 %s
}
