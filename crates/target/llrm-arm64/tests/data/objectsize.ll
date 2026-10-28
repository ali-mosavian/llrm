target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

declare i64 @llvm.objectsize.i64.p0(ptr, i1, i1, i1)

define i64 @maximum(ptr %p) {
  %n = call i64 @llvm.objectsize.i64.p0(ptr %p, i1 false, i1 true, i1 false)
  ret i64 %n
}

define i64 @minimum(ptr %p) {
  %n = call i64 @llvm.objectsize.i64.p0(ptr %p, i1 true, i1 true, i1 false)
  ret i64 %n
}
