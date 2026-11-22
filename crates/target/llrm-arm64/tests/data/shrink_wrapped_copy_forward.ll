target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

define fastcc void @sort(ptr %0, i32 %1, i32 %2) {
  %4 = icmp slt i32 %1, %2
  br i1 %4, label %5, label %35

5:                                                ; preds = %3
  %6 = sext i32 %2 to i64
  %7 = getelementptr inbounds [4 x i8], ptr %0, i64 %6
  br label %8

8:                                                ; preds = %5, %27
  %9 = phi i32 [ %1, %5 ], [ %33, %27 ]
  %10 = load i32, ptr %7, align 4
  %11 = sext i32 %9 to i64
  br label %12

12:                                               ; preds = %8, %23
  %13 = phi i64 [ %11, %8 ], [ %25, %23 ]
  %14 = phi i32 [ %9, %8 ], [ %24, %23 ]
  %15 = getelementptr inbounds [4 x i8], ptr %0, i64 %13
  %16 = load i32, ptr %15, align 4
  %17 = icmp slt i32 %16, %10
  br i1 %17, label %18, label %23

18:                                               ; preds = %12
  %19 = sext i32 %14 to i64
  %20 = getelementptr inbounds [4 x i8], ptr %0, i64 %19
  %21 = load i32, ptr %20, align 4
  store i32 %16, ptr %20, align 4
  store i32 %21, ptr %15, align 4
  %22 = add nsw i32 %14, 1
  br label %23

23:                                               ; preds = %12, %18
  %24 = phi i32 [ %22, %18 ], [ %14, %12 ]
  %25 = add nsw i64 %13, 1
  %26 = icmp slt i64 %25, %6
  br i1 %26, label %12, label %27

27:                                               ; preds = %23
  %28 = sext i32 %24 to i64
  %29 = getelementptr inbounds [4 x i8], ptr %0, i64 %28
  %30 = load i32, ptr %29, align 4
  %31 = load i32, ptr %7, align 4
  store i32 %31, ptr %29, align 4
  store i32 %30, ptr %7, align 4
  %32 = add nsw i32 %24, -1
  tail call fastcc void @sort(ptr %0, i32 %9, i32 %32)
  %33 = add nsw i32 %24, 1
  %34 = icmp slt i32 %33, %2
  br i1 %34, label %8, label %35

35:                                               ; preds = %27, %3
  ret void
}
