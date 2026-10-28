target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx"

declare void @keep(ptr)

define i32 @f(ptr %fn, i32 %x) {
  %a = add i32 %x, 1
  %r = tail call i32 %fn(i32 %a)
  ret i32 %r
}
