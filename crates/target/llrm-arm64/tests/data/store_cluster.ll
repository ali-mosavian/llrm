target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"
@a = global [4 x i64] zeroinitializer, align 8
define void @f() {
  store i64 1234567890123, ptr @a, align 8
  store i64 9876543210987, ptr getelementptr inbounds ([4 x i64], ptr @a, i64 0, i64 1), align 8
  ret void
}
define void @g(ptr %p, i64 %x) {
  %a = mul i64 %x, 3
  store i64 %a, ptr %p, align 8
  %b = mul i64 %x, 5
  %q = getelementptr i64, ptr %p, i64 1
  store i64 %b, ptr %q, align 8
  ret void
}

define void @across_a_store_that_may_alias(ptr %p, ptr %q, i64 %x) {
  %a = mul i64 %x, 3
  store i64 %a, ptr %p, align 8
  store i64 7, ptr %q, align 8
  %b = mul i64 %x, 5
  %r = getelementptr i64, ptr %p, i64 1
  store i64 %b, ptr %r, align 8
  ret void
}
