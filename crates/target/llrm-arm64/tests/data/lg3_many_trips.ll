target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
@a = dso_local global [1024 x i32] zeroinitializer, align 4
@b = dso_local global [1024 x i32] zeroinitializer, align 4
@c = dso_local global [1024 x i32] zeroinitializer, align 4

define dso_local void @f(i32 %n) {
entry:
  %g = icmp sgt i32 %n, 0
  br i1 %g, label %body, label %exit

body:
  %i = phi i64 [ 0, %entry ], [ %i1, %body ]
  %pb = getelementptr inbounds [1024 x i32], ptr @b, i64 0, i64 %i
  %x = load i32, ptr %pb, align 4
  %pc = getelementptr inbounds [1024 x i32], ptr @c, i64 0, i64 %i
  %y = load i32, ptr %pc, align 4
  %s = add i32 %x, %y
  %pa = getelementptr inbounds [1024 x i32], ptr @a, i64 0, i64 %i
  store i32 %s, ptr %pa, align 4
  %i1 = add nuw nsw i64 %i, 1
  %w = zext nneg i32 %n to i64
  %done = icmp eq i64 %i1, %w
  br i1 %done, label %exit, label %body

exit:
  ret void
}
