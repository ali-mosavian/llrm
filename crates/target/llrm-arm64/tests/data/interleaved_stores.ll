target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@a = global [4 x i64] zeroinitializer, align 8
@b = global [4 x i64] zeroinitializer, align 8

define void @fill(i64 %w, i64 %x, i64 %y, i64 %z) {
  store i64 %w, ptr getelementptr inbounds ([4 x i64], ptr @a, i64 0, i64 0), align 8
  store i64 %x, ptr getelementptr inbounds ([4 x i64], ptr @b, i64 0, i64 0), align 8
  store i64 %y, ptr getelementptr inbounds ([4 x i64], ptr @a, i64 0, i64 1), align 8
  store i64 %z, ptr getelementptr inbounds ([4 x i64], ptr @b, i64 0, i64 1), align 8
  ret void
}
