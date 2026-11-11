target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@a = internal global i32 1, align 4
@b = internal global i32 2, align 4
@c = internal global i32 3, align 4
@lone = internal global i32 4, align 4
@d = internal global i32 5, align 4
@e = internal global i32 6, align 4
@big = internal global [2000 x i32] zeroinitializer, align 4

define i32 @three(i32 %x) {
  %p = load volatile i32, ptr @a
  %q = load volatile i32, ptr @b
  %r = load volatile i32, ptr @c
  store volatile i32 %x, ptr @a
  store volatile i32 %x, ptr @b
  store volatile i32 %x, ptr @c
  %s = add i32 %p, %q
  %t = add i32 %s, %r
  ret i32 %t
}

define i32 @alone() {
  %p = load volatile i32, ptr @lone
  store volatile i32 %p, ptr @lone
  ret i32 %p
}

define i32 @large(i32 %i) {
  %e = getelementptr [2000 x i32], ptr @big, i32 0, i32 %i
  %p = load volatile i32, ptr %e
  store volatile i32 %i, ptr %e
  store volatile i32 %i, ptr @a
  %q = load volatile i32, ptr @a
  %s = add i32 %p, %q
  ret i32 %s
}

define i32 @small(i32 %x) minsize {
  %p = load volatile i32, ptr @b
  %q = load volatile i32, ptr @c
  store volatile i32 %x, ptr @b
  store volatile i32 %x, ptr @c
  %s = add i32 %p, %q
  ret i32 %s
}

define i32 @sized(i32 %x) optsize {
  %p = load volatile i32, ptr @d
  %q = load volatile i32, ptr @e
  store volatile i32 %x, ptr @d
  store volatile i32 %x, ptr @e
  %s = add i32 %p, %q
  ret i32 %s
}
