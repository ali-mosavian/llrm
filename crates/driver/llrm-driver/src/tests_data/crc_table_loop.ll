target datalayout = "@m32-datalayout@"

@table = internal global [256 x i32] zeroinitializer, align 4

define dso_local i32 @bench_x_crc32(i32 noundef %0) {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
  store i32 -1, ptr %4, align 4
  store i32 0, ptr %5, align 4
  br label %7

7:                                                ; preds = %35, %1
  %8 = load i32, ptr %5, align 4
  %9 = icmp slt i32 %8, 256
  br i1 %9, label %10, label %38

10:                                               ; preds = %7
  %11 = load i32, ptr %5, align 4
  store i32 %11, ptr %3, align 4
  store i32 0, ptr %6, align 4
  br label %12

12:                                               ; preds = %28, %10
  %13 = load i32, ptr %6, align 4
  %14 = icmp slt i32 %13, 8
  br i1 %14, label %15, label %31

15:                                               ; preds = %12
  %16 = load i32, ptr %3, align 4
  %17 = and i32 %16, 1
  %18 = icmp ne i32 %17, 0
  br i1 %18, label %19, label %23

19:                                               ; preds = %15
  %20 = load i32, ptr %3, align 4
  %21 = lshr i32 %20, 1
  %22 = xor i32 -306674912, %21
  br label %26

23:                                               ; preds = %15
  %24 = load i32, ptr %3, align 4
  %25 = lshr i32 %24, 1
  br label %26

26:                                               ; preds = %23, %19
  %27 = phi i32 [ %22, %19 ], [ %25, %23 ]
  store i32 %27, ptr %3, align 4
  br label %28

28:                                               ; preds = %26
  %29 = load i32, ptr %6, align 4
  %30 = add nsw i32 %29, 1
  store i32 %30, ptr %6, align 4
  br label %12

31:                                               ; preds = %12
  %32 = load i32, ptr %3, align 4
  %33 = load i32, ptr %5, align 4
  %34 = getelementptr inbounds [256 x i32], ptr @table, i32 0, i32 %33
  store i32 %32, ptr %34, align 4
  br label %35

35:                                               ; preds = %31
  %36 = load i32, ptr %5, align 4
  %37 = add nsw i32 %36, 1
  store i32 %37, ptr %5, align 4
  br label %7

38:                                               ; preds = %7
  store i32 0, ptr %5, align 4
  br label %39

39:                                               ; preds = %57, %38
  %40 = load i32, ptr %5, align 4
  %41 = load i32, ptr %2, align 4
  %42 = icmp slt i32 %40, %41
  br i1 %42, label %43, label %60

43:                                               ; preds = %39
  %44 = load i32, ptr %4, align 4
  %45 = load i32, ptr %5, align 4
  %46 = mul nsw i32 %45, 7
  %47 = load i32, ptr %5, align 4
  %48 = ashr i32 %47, 3
  %49 = add nsw i32 %46, %48
  %50 = xor i32 %44, %49
  %51 = and i32 %50, 255
  %52 = getelementptr inbounds nuw [256 x i32], ptr @table, i32 0, i32 %51
  %53 = load i32, ptr %52, align 4
  %54 = load i32, ptr %4, align 4
  %55 = lshr i32 %54, 8
  %56 = xor i32 %53, %55
  store i32 %56, ptr %4, align 4
  br label %57

57:                                               ; preds = %43
  %58 = load i32, ptr %5, align 4
  %59 = add nsw i32 %58, 1
  store i32 %59, ptr %5, align 4
  br label %39

60:                                               ; preds = %39
  %61 = load i32, ptr %4, align 4
  %62 = and i32 %61, 2147483647
  ret i32 %62
}
