target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx"

declare i64 @paths(i64, i64)

define i64 @f(i64 %n) {
  %r = tail call i64 @paths(i64 %n, i64 %n)
  ret i64 %r
}
