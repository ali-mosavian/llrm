target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx"
declare void @report(i64)
define i32 @main() {
  call void @report(i64 550)
  call void @report(i64 0)
  call void @report(i64 1)
  call void @report(i64 0)
  ret i32 0
}
