target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-linux-gnu"

@a = dso_local global i32 0, align 4
@b = dso_local global i32 0, align 4
@c = dso_local global i32 0, align 4
@d = dso_local global i32 0, align 4

define void @f(i32 %x) minsize nounwind {
  store i32 %x, ptr @a
  store i32 %x, ptr @b
  store i32 %x, ptr @c
  store i32 %x, ptr @d
  ret void
}
