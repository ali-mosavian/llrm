target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@pool = global [8 x i64] zeroinitializer, align 8

define void @fill() {
  store i64 1, ptr getelementptr inbounds ([8 x i64], ptr @pool, i64 0, i64 1), align 8
  store i64 2, ptr getelementptr inbounds ([8 x i64], ptr @pool, i64 0, i64 3), align 8
  store i64 3, ptr getelementptr inbounds ([8 x i64], ptr @pool, i64 0, i64 5), align 8
  ret void
}
