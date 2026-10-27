target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx"

@a = internal global [64 x i16] zeroinitializer, align 2
@b = internal global [64 x i16] zeroinitializer, align 2

declare void @report(i64)

define i32 @main() {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %t = trunc i64 %i to i16
  %x = add i16 %t, 100
  %y = add i16 %t, 200
  %pa = getelementptr [64 x i16], ptr @a, i64 0, i64 %i
  %pb = getelementptr [64 x i16], ptr @b, i64 0, i64 %i
  store i16 %x, ptr %pa, align 2
  store i16 %y, ptr %pb, align 2
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 64
  br i1 %done, label %exit, label %loop
exit:
  %v = load i16, ptr getelementptr ([64 x i16], ptr @a, i64 0, i64 7), align 2
  %w = zext i16 %v to i64
  call void @report(i64 %w)
  ret i32 0
}
