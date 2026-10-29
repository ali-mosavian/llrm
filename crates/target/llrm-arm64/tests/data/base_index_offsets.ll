target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

define void @fill(ptr %p, i64 %i) {
  %q = getelementptr i8, ptr %p, i64 %i
  %a = getelementptr i8, ptr %q, i64 8
  %b = getelementptr i8, ptr %q, i64 9
  %c = getelementptr i8, ptr %q, i64 16
  store i8 1, ptr %a, align 1
  store i8 0, ptr %b, align 1
  store i64 0, ptr %c, align 8
  ret void
}
