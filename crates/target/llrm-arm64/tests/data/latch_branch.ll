target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

define i64 @power(i64 noundef %0, i32 noundef %1) {
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  %5 = alloca i32, align 4
  store i64 %0, ptr %4, align 8
  store i32 %1, ptr %5, align 4
  %6 = load i32, ptr %5, align 4
  %7 = icmp eq i32 %6, 0
  br i1 %7, label %8, label %9

8:                                                ; preds = %2
  store i64 1, ptr %3, align 8
  br label %29

9:                                                ; preds = %2
  %10 = load i32, ptr %5, align 4
  %11 = and i32 %10, 1
  %12 = icmp ne i32 %11, 0
  br i1 %12, label %13, label %21

13:                                               ; preds = %9
  %14 = load i64, ptr %4, align 8
  %15 = load i64, ptr %4, align 8
  %16 = load i32, ptr %5, align 4
  %17 = sub nsw i32 %16, 1
  %18 = call i64 @power(i64 noundef %15, i32 noundef %17)
  %19 = mul nsw i64 %14, %18
  %20 = srem i64 %19, 30011
  store i64 %20, ptr %3, align 8
  br label %29

21:                                               ; preds = %9
  %22 = load i64, ptr %4, align 8
  %23 = load i64, ptr %4, align 8
  %24 = mul nsw i64 %22, %23
  %25 = srem i64 %24, 30011
  %26 = load i32, ptr %5, align 4
  %27 = ashr i32 %26, 1
  %28 = call i64 @power(i64 noundef %25, i32 noundef %27)
  store i64 %28, ptr %3, align 8
  br label %29

29:                                               ; preds = %21, %13, %8
  %30 = load i64, ptr %3, align 8
  ret i64 %30
}

