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

define i64 @a_chain_of_loads(ptr %p, i64 %s) {
  %a = load i64, ptr %p, align 8
  %a3 = mul i64 %a, 3
  %t1 = add i64 %s, %a3
  %q1 = getelementptr i64, ptr %p, i64 1
  %b = load i64, ptr %q1, align 8
  %b5 = mul i64 %b, 5
  %t2 = add i64 %t1, %b5
  %q2 = getelementptr i64, ptr %p, i64 2
  %c = load i64, ptr %q2, align 8
  %c7 = mul i64 %c, 7
  %t3 = add i64 %t2, %c7
  %q3 = getelementptr i64, ptr %p, i64 3
  %d = load i64, ptr %q3, align 8
  %d9 = mul i64 %d, 9
  %t4 = add i64 %t3, %d9
  ret i64 %t4
}

define i64 @loads_across_a_store_that_may_alias(ptr %p, ptr %s) {
  %a = load i64, ptr %p, align 8
  %m = mul i64 %a, 3
  store i64 7, ptr %s, align 8
  %q = getelementptr i64, ptr %p, i64 1
  %b = load i64, ptr %q, align 8
  %n = mul i64 %b, 5
  %r = add i64 %m, %n
  ret i64 %r
}

declare void @sink(ptr)
define i64 @a_chain_of_slot_loads(i64 %s) {
  %p = alloca [4 x i64], align 8
  call void @sink(ptr %p)
  %a = load i64, ptr %p, align 8
  %a3 = mul i64 %a, 3
  %t1 = add i64 %s, %a3
  %q1 = getelementptr i64, ptr %p, i64 1
  %b = load i64, ptr %q1, align 8
  %b5 = mul i64 %b, 5
  %t2 = add i64 %t1, %b5
  %q2 = getelementptr i64, ptr %p, i64 2
  %c = load i64, ptr %q2, align 8
  %c7 = mul i64 %c, 7
  %t3 = add i64 %t2, %c7
  %q3 = getelementptr i64, ptr %p, i64 3
  %d = load i64, ptr %q3, align 8
  %d9 = mul i64 %d, 9
  %t4 = add i64 %t3, %d9
  ret i64 %t4
}

define float @slot_stores_interleaved(float %x, float %y) {
  %p = alloca [2 x float], align 4
  %r = alloca [2 x float], align 4
  call void @sink(ptr %p)
  call void @sink(ptr %r)
  %a = fadd float %x, 1.0
  store float %a, ptr %p, align 4
  %b = fadd float %y, 2.0
  store float %b, ptr %r, align 4
  %c = fmul float %x, %y
  %p1 = getelementptr float, ptr %p, i64 1
  store float %c, ptr %p1, align 4
  %d = fsub float %x, %y
  %r1 = getelementptr float, ptr %r, i64 1
  store float %d, ptr %r1, align 4
  call void @sink(ptr %p)
  call void @sink(ptr %r)
  ret float %a
}
