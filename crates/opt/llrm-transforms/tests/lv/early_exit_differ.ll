target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

@a = global [1200 x i8] zeroinitializer
@b = global [1200 x i8] zeroinitializer

define i32 @f() {
entry:
  br label %body
body:
  %i = phi i64 [ 0, %entry ], [ %i.next, %latch ]
  %pa = getelementptr inbounds i8, ptr @a, i64 %i
  %pb = getelementptr inbounds i8, ptr @b, i64 %i
  %x = load i8, ptr %pa, align 1
  %y = load i8, ptr %pb, align 1
  %ne = icmp ne i8 %x, %y
  br i1 %ne, label %found, label %latch
latch:
  %i.next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %i.next, 1200
  br i1 %done, label %same, label %body
found:
  ret i32 1
same:
  ret i32 0
}
