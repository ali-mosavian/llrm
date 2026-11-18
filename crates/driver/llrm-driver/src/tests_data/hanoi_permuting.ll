target datalayout = "@m32-datalayout@"

@peg = internal global [3 x [16 x i32]] zeroinitializer, align 4
@top = internal global [3 x i32] zeroinitializer, align 4
@moves = internal global i32 0, align 4

; Function Attrs: nounwind uwtable
define dso_local i32 @bench_x_hanoi2(i32 noundef %0) {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
  call void @llvm.lifetime.start.p0(ptr %3)
  store i32 0, ptr %3, align 4
  br label %4

4:                                                ; preds = %15, %1
  %5 = load i32, ptr %3, align 4
  %6 = load i32, ptr %2, align 4
  %7 = icmp slt i32 %5, %6
  br i1 %7, label %8, label %18

8:                                                ; preds = %4
  %9 = load i32, ptr %2, align 4
  %10 = load i32, ptr %3, align 4
  %11 = sub nsw i32 %9, %10
  %12 = load i32, ptr @top, align 4
  %13 = add nsw i32 %12, 1
  store i32 %13, ptr @top, align 4
  %14 = getelementptr inbounds [16 x i32], ptr @peg, i32 0, i32 %12
  store i32 %11, ptr %14, align 4
  br label %15

15:                                               ; preds = %8
  %16 = load i32, ptr %3, align 4
  %17 = add nsw i32 %16, 1
  store i32 %17, ptr %3, align 4
  br label %4

18:                                               ; preds = %4
  %19 = load i32, ptr %2, align 4
  call void @mv(i32 noundef %19, i32 noundef 0, i32 noundef 2, i32 noundef 1)
  %20 = load i32, ptr @moves, align 4
  %21 = mul nsw i32 %20, 100
  %22 = load i32, ptr getelementptr inbounds nuw (i8, ptr @top, i32 8), align 4
  %23 = add nsw i32 %21, %22
  call void @llvm.lifetime.end.p0(ptr %3)
  ret i32 %23
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none))

; Function Attrs: nounwind uwtable
define internal void @mv(i32 noundef %0, i32 noundef %1, i32 noundef %2, i32 noundef %3) {
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  %8 = alloca i32, align 4
  store i32 %0, ptr %5, align 4
  store i32 %1, ptr %6, align 4
  store i32 %2, ptr %7, align 4
  store i32 %3, ptr %8, align 4
  %9 = load i32, ptr %5, align 4
  %10 = icmp eq i32 %9, 0
  br i1 %10, label %11, label %12

11:                                               ; preds = %4
  br label %40

12:                                               ; preds = %4
  %13 = load i32, ptr %5, align 4
  %14 = sub nsw i32 %13, 1
  %15 = load i32, ptr %6, align 4
  %16 = load i32, ptr %8, align 4
  %17 = load i32, ptr %7, align 4
  call void @mv(i32 noundef %14, i32 noundef %15, i32 noundef %16, i32 noundef %17)
  %18 = load i32, ptr %6, align 4
  %19 = getelementptr inbounds [3 x [16 x i32]], ptr @peg, i32 0, i32 %18
  %20 = load i32, ptr %6, align 4
  %21 = getelementptr inbounds [3 x i32], ptr @top, i32 0, i32 %20
  %22 = load i32, ptr %21, align 4
  %23 = add nsw i32 %22, -1
  store i32 %23, ptr %21, align 4
  %24 = getelementptr inbounds [16 x i32], ptr %19, i32 0, i32 %23
  %25 = load i32, ptr %24, align 4
  %26 = load i32, ptr %7, align 4
  %27 = getelementptr inbounds [3 x [16 x i32]], ptr @peg, i32 0, i32 %26
  %28 = load i32, ptr %7, align 4
  %29 = getelementptr inbounds [3 x i32], ptr @top, i32 0, i32 %28
  %30 = load i32, ptr %29, align 4
  %31 = add nsw i32 %30, 1
  store i32 %31, ptr %29, align 4
  %32 = getelementptr inbounds [16 x i32], ptr %27, i32 0, i32 %30
  store i32 %25, ptr %32, align 4
  %33 = load i32, ptr @moves, align 4
  %34 = add nsw i32 %33, 1
  store i32 %34, ptr @moves, align 4
  %35 = load i32, ptr %5, align 4
  %36 = sub nsw i32 %35, 1
  %37 = load i32, ptr %8, align 4
  %38 = load i32, ptr %7, align 4
  %39 = load i32, ptr %6, align 4
  call void @mv(i32 noundef %36, i32 noundef %37, i32 noundef %38, i32 noundef %39)
  br label %40

40:                                               ; preds = %12, %11
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none))

; Function Attrs: nounwind uwtable
define dso_local i32 @main() {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = call i32 @bench_x_hanoi2(i32 noundef 12)
  call void @report(i32 noundef %2)
  ret i32 0
}

declare void @report(i32 noundef)

