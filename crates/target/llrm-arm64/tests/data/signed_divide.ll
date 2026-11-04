target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

define i32 @by_seven(i32 %x) {
  %d = sdiv i32 %x, 7
  ret i32 %d
}

define i32 @by_five(i32 %x) {
  %d = sdiv i32 %x, 5
  ret i32 %d
}
