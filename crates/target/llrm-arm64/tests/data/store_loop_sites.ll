target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@bmp = internal global [64 x i8] zeroinitializer

define internal void @put32(i32 %at, i64 %value) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %shift = shl i32 %i, 3
  %wide = zext i32 %shift to i64
  %byte = lshr i64 %value, %wide
  %low = trunc i64 %byte to i8
  %index = add i32 %at, %i
  %where = zext i32 %index to i64
  %slot = getelementptr inbounds [64 x i8], ptr @bmp, i64 0, i64 %where
  store volatile i8 %low, ptr %slot
  %next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %next, 4
  br i1 %done, label %exit, label %loop
exit:
  ret void
}

define void @header(i64 %size, i64 %width, i64 %height) optsize {
entry:
  call void @put32(i32 2, i64 %size)
  call void @put32(i32 10, i64 54)
  call void @put32(i32 14, i64 40)
  call void @put32(i32 18, i64 %width)
  call void @put32(i32 22, i64 %height)
  call void @put32(i32 34, i64 %size)
  ret void
}
