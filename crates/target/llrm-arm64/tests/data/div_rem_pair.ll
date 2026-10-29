target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx"

declare void @use(i64, i64)

define void @f(i64 %x, i64 %y, i1 %c) {
entry:
  br i1 %c, label %rem, label %div
rem:
  %r = srem i64 %x, %y
  call void @use(i64 %r, i64 0)
  br label %div
div:
  %q = sdiv i64 %x, %y
  call void @use(i64 %q, i64 1)
  ret void
}
