target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx"

declare i32 @g(i32)

define i32 @jump(i32 %x) {
  %a = add i32 %x, 1
  %r = tail call i32 @g(i32 %a)
  ret i32 %r
}

define i32 @plain(i32 %x) {
  %a = add i32 %x, 1
  %r = notail call i32 @g(i32 %a)
  ret i32 %r
}

define i32 @changed(i32 %x) {
  %r = tail call i32 @g(i32 %x)
  %s = add i32 %r, 1
  ret i32 %s
}
