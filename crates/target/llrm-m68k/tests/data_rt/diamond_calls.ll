; ModuleID = 'e.c'
source_filename = "e.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

@diamonds.xs = internal constant [2 x i32] [i32 19088743, i32 -2147483647], align 2
@acc = internal global i32 0, align 2

; Function Attrs: noinline nounwind
define dso_local i32 @diamonds() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  store i32 0, ptr %2, align 2
  br label %4

4:                                                ; preds = %51, %0
  %5 = load i32, ptr %2, align 2
  %6 = icmp slt i32 %5, 2
  br i1 %6, label %7, label %54

7:                                                ; preds = %4
  store i32 0, ptr %3, align 2
  br label %8

8:                                                ; preds = %47, %7
  %9 = load i32, ptr %3, align 2
  %10 = icmp slt i32 %9, 4
  br i1 %10, label %11, label %50

11:                                               ; preds = %8
  %12 = load i32, ptr %3, align 2
  store volatile i32 %12, ptr %1, align 2
  %13 = load i32, ptr %3, align 2
  %14 = and i32 %13, 1
  %15 = icmp ne i32 %14, 0
  br i1 %15, label %16, label %22

16:                                               ; preds = %11
  %17 = load i32, ptr %2, align 2
  %18 = getelementptr inbounds [2 x i32], ptr @diamonds.xs, i32 0, i32 %17
  %19 = load i32, ptr %18, align 2
  %20 = load volatile i32, ptr %1, align 2
  %21 = call i32 @a2(i32 noundef %19, i32 noundef %20) #1
  br label %28

22:                                               ; preds = %11
  %23 = load i32, ptr %2, align 2
  %24 = getelementptr inbounds [2 x i32], ptr @diamonds.xs, i32 0, i32 %23
  %25 = load i32, ptr %24, align 2
  %26 = load volatile i32, ptr %1, align 2
  %27 = call i32 @a1(i32 noundef %25, i32 noundef %26) #1
  br label %28

28:                                               ; preds = %22, %16
  %29 = phi i32 [ %21, %16 ], [ %27, %22 ]
  call void @report(i32 noundef %29) #1
  %30 = load i32, ptr %3, align 2
  %31 = and i32 %30, 1
  %32 = icmp ne i32 %31, 0
  br i1 %32, label %33, label %39

33:                                               ; preds = %28
  %34 = load i32, ptr %2, align 2
  %35 = getelementptr inbounds [2 x i32], ptr @diamonds.xs, i32 0, i32 %34
  %36 = load i32, ptr %35, align 2
  %37 = load volatile i32, ptr %1, align 2
  %38 = call i32 @b2(i32 noundef %36, i32 noundef %37) #1
  br label %45

39:                                               ; preds = %28
  %40 = load i32, ptr %2, align 2
  %41 = getelementptr inbounds [2 x i32], ptr @diamonds.xs, i32 0, i32 %40
  %42 = load i32, ptr %41, align 2
  %43 = load volatile i32, ptr %1, align 2
  %44 = call i32 @b1(i32 noundef %42, i32 noundef %43) #1
  br label %45

45:                                               ; preds = %39, %33
  %46 = phi i32 [ %38, %33 ], [ %44, %39 ]
  call void @report(i32 noundef %46) #1
  br label %47

47:                                               ; preds = %45
  %48 = load i32, ptr %3, align 2
  %49 = add nsw i32 %48, 1
  store i32 %49, ptr %3, align 2
  br label %8

50:                                               ; preds = %8
  br label %51

51:                                               ; preds = %50
  %52 = load i32, ptr %2, align 2
  %53 = add nsw i32 %52, 1
  store i32 %53, ptr %2, align 2
  br label %4

54:                                               ; preds = %4
  %55 = load i32, ptr @acc, align 2
  %56 = and i32 %55, 255
  ret i32 %56
}

; Function Attrs: noinline nounwind
define internal void @report(i32 noundef %0) #0 {
  %2 = alloca i32, align 2
  store i32 %0, ptr %2, align 2
  %3 = load i32, ptr %2, align 2
  %4 = load i32, ptr @acc, align 2
  %5 = add nsw i32 %4, %3
  store i32 %5, ptr @acc, align 2
  ret void
}

; Function Attrs: noinline nounwind
define internal i32 @a2(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2
  store i32 %1, ptr %4, align 2
  %5 = load i32, ptr %3, align 2
  %6 = load i32, ptr %4, align 2
  %7 = shl i32 %5, %6
  ret i32 %7
}

; Function Attrs: noinline nounwind
define internal i32 @a1(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2
  store i32 %1, ptr %4, align 2
  %5 = load i32, ptr %3, align 2
  %6 = load i32, ptr %4, align 2
  %7 = shl i32 %5, %6
  ret i32 %7
}

; Function Attrs: noinline nounwind
define internal i32 @b2(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2
  store i32 %1, ptr %4, align 2
  %5 = load i32, ptr %3, align 2
  %6 = load i32, ptr %4, align 2
  %7 = lshr i32 %5, %6
  ret i32 %7
}

; Function Attrs: noinline nounwind
define internal i32 @b1(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2
  store i32 %1, ptr %4, align 2
  %5 = load i32, ptr %3, align 2
  %6 = load i32, ptr %4, align 2
  %7 = lshr i32 %5, %6
  ret i32 %7
}

attributes #0 = { noinline nounwind "frame-pointer"="all" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #1 = { nobuiltin "no-builtins" }


