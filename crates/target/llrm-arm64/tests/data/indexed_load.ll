target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"
define i32 @f(ptr %p, ptr %q) {
  %a = getelementptr inbounds i8, ptr %p, i64 16
  %v = load i32, ptr %a, align 4
  store ptr %a, ptr %q, align 8
  ret i32 %v
}
define i32 @g(ptr %p, ptr %q) {
  %v = load i32, ptr %p, align 4
  %a = getelementptr inbounds i8, ptr %p, i64 16
  store ptr %a, ptr %q, align 8
  ret i32 %v
}
