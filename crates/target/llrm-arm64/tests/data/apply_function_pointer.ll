target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx"

declare i32 @printf(ptr, ...)
@.str = private constant [7 x i8] c"%d %d\0A\00"

define internal i32 @apply(ptr %f, i32 %a, i32 %b) {
  %r = call i32 %f(i32 %a, i32 %b)
  ret i32 %r
}
define internal i32 @plus(i32 %a, i32 %b) {
  %r = add nsw i32 %a, %b
  ret i32 %r
}
define internal i32 @times(i32 %a, i32 %b) {
  %r = mul nsw i32 %a, %b
  ret i32 %r
}
define i32 @main() {
  %v = call i32 @apply(ptr @plus, i32 6, i32 7)
  %w = call i32 @apply(ptr @times, i32 6, i32 7)
  %p = call i32 (ptr, ...) @printf(ptr @.str, i32 %v, i32 %w)
  ret i32 0
}
