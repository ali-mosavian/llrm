target datalayout = "@m32-datalayout@"

@values = internal global [64 x i32] zeroinitializer, align 4

; Function Attrs: nounwind uwtable
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

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

; Function Attrs: nounwind uwtable
define internal i32 @chop(i32 noundef %0, i32 noundef %1) {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  %8 = alloca i32, align 4
  %9 = alloca i32, align 4
  store i32 %0, ptr %4, align 4
  store i32 %1, ptr %5, align 4
  store i32 0, ptr %6, align 4
  %10 = load i32, ptr %5, align 4
  %11 = load i32, ptr %4, align 4
  %12 = sub nsw i32 %10, %11
  %13 = icmp slt i32 %12, 2
  br i1 %13, label %14, label %15

14:                                               ; preds = %2
  store i32 0, ptr %3, align 4
  store i32 1, ptr %9, align 4
  br label %46

15:                                               ; preds = %2
  %16 = load i32, ptr %4, align 4
  %17 = load i32, ptr %5, align 4
  %18 = add nsw i32 %16, %17
  %19 = sdiv i32 %18, 2
  store i32 %19, ptr %8, align 4
  %20 = load i32, ptr %4, align 4
  store i32 %20, ptr %7, align 4
  br label %21

21:                                               ; preds = %33, %15
  %22 = load i32, ptr %7, align 4
  %23 = load i32, ptr %5, align 4
  %24 = icmp slt i32 %22, %23
  br i1 %24, label %25, label %36

25:                                               ; preds = %21
  %26 = load i32, ptr %7, align 4
  %27 = getelementptr inbounds [64 x i32], ptr @values, i32 0, i32 %26
  %28 = load i32, ptr %27, align 4
  %29 = load i32, ptr %8, align 4
  %30 = and i32 %28, %29
  %31 = load i32, ptr %6, align 4
  %32 = add nsw i32 %31, %30
  store i32 %32, ptr %6, align 4
  br label %33

33:                                               ; preds = %25
  %34 = load i32, ptr %7, align 4
  %35 = add nsw i32 %34, 1
  store i32 %35, ptr %7, align 4
  br label %21

36:                                               ; preds = %21
  %37 = load i32, ptr %6, align 4
  %38 = load i32, ptr %4, align 4
  %39 = load i32, ptr %8, align 4
  %40 = call i32 @chop(i32 noundef %38, i32 noundef %39)
  %41 = add nsw i32 %37, %40
  %42 = load i32, ptr %8, align 4
  %43 = load i32, ptr %5, align 4
  %44 = call i32 @chop(i32 noundef %42, i32 noundef %43)
  %45 = add nsw i32 %41, %44
  store i32 %45, ptr %3, align 4
  store i32 1, ptr %9, align 4
  br label %46

46:                                               ; preds = %36, %14
  %47 = load i32, ptr %3, align 4
  ret i32 %47
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

; Function Attrs: nounwind uwtable
define dso_local i32 @main() {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = call i32 @bench_recchop(i32 noundef 5)
  call void @report(i32 noundef %2)
  ret i32 0
}

declare void @report(i32 noundef)

