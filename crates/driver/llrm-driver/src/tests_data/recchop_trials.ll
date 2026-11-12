target datalayout = "@m32-datalayout@"

@values = internal global [64 x i32] zeroinitializer, align 4

define dso_local i32 @bench_recchop(i32 noundef %0) {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
  store i32 0, ptr %3, align 4
  br label %4

4:                                                ; preds = %15, %1
  %5 = load i32, ptr %3, align 4
  %6 = icmp slt i32 %5, 64
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %3, align 4
  %9 = mul nsw i32 %8, 37
  %10 = load i32, ptr %2, align 4
  %11 = add nsw i32 %9, %10
  %12 = and i32 %11, 127
  %13 = load i32, ptr %3, align 4
  %14 = getelementptr inbounds [64 x i32], ptr @values, i32 0, i32 %13
  store i32 %12, ptr %14, align 4
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %3, align 4
  %17 = add nsw i32 %16, 1
  store i32 %17, ptr %3, align 4
  br label %4

18:                                               ; preds = %4
  %19 = call i32 @chop(i32 noundef 0, i32 noundef 64)
  ret i32 %19
}
define internal i32 @chop(i32 noundef %0, i32 noundef %1) {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  %8 = alloca i32, align 4
  store i32 %0, ptr %4, align 4
  store i32 %1, ptr %5, align 4
  store i32 0, ptr %6, align 4
  %9 = load i32, ptr %5, align 4
  %10 = load i32, ptr %4, align 4
  %11 = sub nsw i32 %9, %10
  %12 = icmp slt i32 %11, 2
  br i1 %12, label %13, label %14

13:                                               ; preds = %2
  store i32 0, ptr %3, align 4
  br label %45

14:                                               ; preds = %2
  %15 = load i32, ptr %4, align 4
  %16 = load i32, ptr %5, align 4
  %17 = add nsw i32 %15, %16
  %18 = sdiv i32 %17, 2
  store i32 %18, ptr %8, align 4
  %19 = load i32, ptr %4, align 4
  store i32 %19, ptr %7, align 4
  br label %20

20:                                               ; preds = %32, %14
  %21 = load i32, ptr %7, align 4
  %22 = load i32, ptr %5, align 4
  %23 = icmp slt i32 %21, %22
  br i1 %23, label %24, label %35

24:                                               ; preds = %20
  %25 = load i32, ptr %7, align 4
  %26 = getelementptr inbounds [64 x i32], ptr @values, i32 0, i32 %25
  %27 = load i32, ptr %26, align 4
  %28 = load i32, ptr %8, align 4
  %29 = and i32 %27, %28
  %30 = load i32, ptr %6, align 4
  %31 = add nsw i32 %30, %29
  store i32 %31, ptr %6, align 4
  br label %32

32:                                               ; preds = %24
  %33 = load i32, ptr %7, align 4
  %34 = add nsw i32 %33, 1
  store i32 %34, ptr %7, align 4
  br label %20

35:                                               ; preds = %20
  %36 = load i32, ptr %6, align 4
  %37 = load i32, ptr %4, align 4
  %38 = load i32, ptr %8, align 4
  %39 = call i32 @chop(i32 noundef %37, i32 noundef %38)
  %40 = add nsw i32 %36, %39
  %41 = load i32, ptr %8, align 4
  %42 = load i32, ptr %5, align 4
  %43 = call i32 @chop(i32 noundef %41, i32 noundef %42)
  %44 = add nsw i32 %40, %43
  store i32 %44, ptr %3, align 4
  br label %45

45:                                               ; preds = %35, %13
  %46 = load i32, ptr %3, align 4
  ret i32 %46
}
