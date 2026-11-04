target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

define internal i64 @twice(i64 %x) {
  %y = add i64 %x, %x
  ret i64 %y
}

define i64 @f(i64 %a) {
  %r = call i64 @twice(i64 %a)
  ret i64 %r
}
