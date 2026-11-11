target datalayout = "@m32-datalayout@"

; Function Attrs: nounwind uwtable
define dso_local i32 @bench_queens(i32 noundef %0) {
  %2 = alloca i32, align 4
  %3 = alloca [12 x i32], align 4
  store i32 %0, ptr %2, align 4
  %4 = getelementptr inbounds [12 x i32], ptr %3, i32 0, i32 0
  %5 = load i32, ptr %2, align 4
  %6 = call i32 @place(ptr noundef %4, i32 noundef 0, i32 noundef %5)
  ret i32 %6
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

; Function Attrs: nounwind uwtable
define internal i32 @place(ptr noundef %0, i32 noundef %1, i32 noundef %2) {
  %4 = alloca i32, align 4
  %5 = alloca ptr, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  %8 = alloca i32, align 4
  %9 = alloca i32, align 4
  %10 = alloca i32, align 4
  store ptr %0, ptr %5, align 4
  store i32 %1, ptr %6, align 4
  store i32 %2, ptr %7, align 4
  store i32 0, ptr %9, align 4
  %11 = load i32, ptr %6, align 4
  %12 = load i32, ptr %7, align 4
  %13 = icmp eq i32 %11, %12
  br i1 %13, label %14, label %15

14:                                               ; preds = %3
  store i32 1, ptr %4, align 4
  store i32 1, ptr %10, align 4
  br label %44

15:                                               ; preds = %3
  store i32 0, ptr %8, align 4
  br label %16

16:                                               ; preds = %39, %15
  %17 = load i32, ptr %8, align 4
  %18 = load i32, ptr %7, align 4
  %19 = icmp slt i32 %17, %18
  br i1 %19, label %20, label %42

20:                                               ; preds = %16
  %21 = load ptr, ptr %5, align 4
  %22 = load i32, ptr %6, align 4
  %23 = load i32, ptr %8, align 4
  %24 = call i32 @safe(ptr noundef %21, i32 noundef %22, i32 noundef %23)
  %25 = icmp ne i32 %24, 0
  br i1 %25, label %26, label %38

26:                                               ; preds = %20
  %27 = load i32, ptr %8, align 4
  %28 = load ptr, ptr %5, align 4
  %29 = load i32, ptr %6, align 4
  %30 = getelementptr inbounds i32, ptr %28, i32 %29
  store i32 %27, ptr %30, align 4
  %31 = load ptr, ptr %5, align 4
  %32 = load i32, ptr %6, align 4
  %33 = add nsw i32 %32, 1
  %34 = load i32, ptr %7, align 4
  %35 = call i32 @place(ptr noundef %31, i32 noundef %33, i32 noundef %34)
  %36 = load i32, ptr %9, align 4
  %37 = add nsw i32 %36, %35
  store i32 %37, ptr %9, align 4
  br label %38

38:                                               ; preds = %26, %20
  br label %39

39:                                               ; preds = %38
  %40 = load i32, ptr %8, align 4
  %41 = add nsw i32 %40, 1
  store i32 %41, ptr %8, align 4
  br label %16

42:                                               ; preds = %16
  %43 = load i32, ptr %9, align 4
  store i32 %43, ptr %4, align 4
  store i32 1, ptr %10, align 4
  br label %44

44:                                               ; preds = %42, %14
  %45 = load i32, ptr %4, align 4
  ret i32 %45
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

; Function Attrs: nounwind uwtable
define dso_local i32 @main() {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = call i32 @bench_queens(i32 noundef 7)
  call void @report(i32 noundef %2)
  ret i32 0
}

declare void @report(i32 noundef)

; Function Attrs: nounwind uwtable
define internal i32 @safe(ptr noundef %0, i32 noundef %1, i32 noundef %2) {
  %4 = alloca i32, align 4
  %5 = alloca ptr, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  %8 = alloca i32, align 4
  %9 = alloca i32, align 4
  %10 = alloca i32, align 4
  store ptr %0, ptr %5, align 4
  store i32 %1, ptr %6, align 4
  store i32 %2, ptr %7, align 4
  store i32 0, ptr %8, align 4
  br label %11

11:                                               ; preds = %41, %3
  %12 = load i32, ptr %8, align 4
  %13 = load i32, ptr %6, align 4
  %14 = icmp slt i32 %12, %13
  br i1 %14, label %15, label %44

15:                                               ; preds = %11
  %16 = load ptr, ptr %5, align 4
  %17 = load i32, ptr %8, align 4
  %18 = getelementptr inbounds i32, ptr %16, i32 %17
  %19 = load i32, ptr %18, align 4
  %20 = load i32, ptr %7, align 4
  %21 = sub nsw i32 %19, %20
  store i32 %21, ptr %9, align 4
  %22 = load i32, ptr %9, align 4
  %23 = icmp eq i32 %22, 0
  br i1 %23, label %36, label %24

24:                                               ; preds = %15
  %25 = load i32, ptr %9, align 4
  %26 = load i32, ptr %6, align 4
  %27 = load i32, ptr %8, align 4
  %28 = sub nsw i32 %26, %27
  %29 = icmp eq i32 %25, %28
  br i1 %29, label %36, label %30

30:                                               ; preds = %24
  %31 = load i32, ptr %9, align 4
  %32 = load i32, ptr %8, align 4
  %33 = load i32, ptr %6, align 4
  %34 = sub nsw i32 %32, %33
  %35 = icmp eq i32 %31, %34
  br i1 %35, label %36, label %37

36:                                               ; preds = %30, %24, %15
  store i32 0, ptr %4, align 4
  store i32 1, ptr %10, align 4
  br label %38

37:                                               ; preds = %30
  store i32 0, ptr %10, align 4
  br label %38

38:                                               ; preds = %37, %36
  %39 = load i32, ptr %10, align 4
  switch i32 %39, label %45 [
    i32 0, label %40
  ]

40:                                               ; preds = %38
  br label %41

41:                                               ; preds = %40
  %42 = load i32, ptr %8, align 4
  %43 = add nsw i32 %42, 1
  store i32 %43, ptr %8, align 4
  br label %11

44:                                               ; preds = %11
  store i32 1, ptr %4, align 4
  store i32 1, ptr %10, align 4
  br label %45

45:                                               ; preds = %44, %38
  %46 = load i32, ptr %4, align 4
  ret i32 %46
}

