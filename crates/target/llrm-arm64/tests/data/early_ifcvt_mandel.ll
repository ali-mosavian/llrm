target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

define dso_local i64 @bench_mandel() nounwind {
  br label %1

1:                                                ; preds = %0, %38
  %2 = phi i64 [ -12, %0 ], [ %39, %38 ]
  %3 = phi i64 [ 0, %0 ], [ %35, %38 ]
  %4 = mul nsw i64 %2, 24
  br label %5

5:                                                ; preds = %1, %32
  %6 = phi i64 [ -16, %1 ], [ %36, %32 ]
  %7 = phi i64 [ %3, %1 ], [ %35, %32 ]
  %8 = mul nsw i64 %6, 24
  %9 = add nsw i64 %8, -128
  br label %10

10:                                               ; preds = %5, %29
  %11 = phi i64 [ 0, %5 ], [ %28, %29 ]
  %12 = phi i64 [ 0, %5 ], [ %27, %29 ]
  %13 = phi i16 [ 0, %5 ], [ %30, %29 ]
  %14 = mul nsw i64 %12, %12
  %15 = lshr i64 %14, 8
  %16 = mul nsw i64 %11, %11
  %17 = lshr i64 %16, 8
  %18 = add nuw nsw i64 %17, %15
  %19 = icmp samesign ult i64 %18, 1025
  br i1 %19, label %20, label %26

20:                                               ; preds = %10
  %21 = mul nsw i64 %11, %12
  %22 = ashr i64 %21, 7
  %23 = add nsw i64 %22, %4
  %24 = add nsw i64 %9, %15
  %25 = sub nsw i64 %24, %17
  br label %26

26:                                               ; preds = %10, %20
  %27 = phi i64 [ %25, %20 ], [ %12, %10 ]
  %28 = phi i64 [ %23, %20 ], [ %11, %10 ]
  br i1 %19, label %29, label %32

29:                                               ; preds = %26
  %30 = add nuw nsw i16 %13, 1
  %31 = icmp samesign ult i16 %13, 31
  br i1 %31, label %10, label %32

32:                                               ; preds = %26, %29
  %33 = phi i16 [ %13, %26 ], [ 32, %29 ]
  %34 = zext nneg i16 %33 to i64
  %35 = add nsw i64 %7, %34
  %36 = add nsw i64 %6, 1
  %37 = icmp eq i64 %36, 16
  br i1 %37, label %38, label %5

38:                                               ; preds = %32
  %39 = add nsw i64 %2, 1
  %40 = icmp eq i64 %39, 12
  br i1 %40, label %41, label %1

41:                                               ; preds = %38
  ret i64 %35
}
