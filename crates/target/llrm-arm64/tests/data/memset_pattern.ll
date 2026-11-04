target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx"

declare void @llvm.experimental.memset.pattern.p0.i16.i64(ptr, i16, i64, i1)

define void @fill(ptr %p, i64 %n) {
  call void @llvm.experimental.memset.pattern.p0.i16.i64(ptr %p, i16 1824, i64 %n, i1 false)
  ret void
}
