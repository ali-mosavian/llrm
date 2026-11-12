target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

define void @copy3(ptr noalias %d, ptr noalias %s) {
  %a = load i8, ptr %s
  store i8 %a, ptr %d
  %s1 = getelementptr i8, ptr %s, i64 1
  %d1 = getelementptr i8, ptr %d, i64 1
  %b = load i8, ptr %s1
  store i8 %b, ptr %d1
  %s2 = getelementptr i8, ptr %s, i64 2
  %d2 = getelementptr i8, ptr %d, i64 2
  %c = load i8, ptr %s2
  store i8 %c, ptr %d2
  ret void
}
