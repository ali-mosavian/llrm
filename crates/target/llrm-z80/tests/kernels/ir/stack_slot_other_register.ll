target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
declare void @llvm.lifetime.start.p0(ptr)
declare void @llvm.lifetime.end.p0(ptr)
define internal i32 @sum(ptr noundef %0, i16 noundef %1) optsize {
  %3 = alloca ptr, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i16, align 2
  store ptr %0, ptr %3, align 2
  store i16 %1, ptr %4, align 2
  call void @llvm.lifetime.start.p0(ptr %5)
  store i32 0, ptr %5, align 2
  call void @llvm.lifetime.start.p0(ptr %6)
  store i16 0, ptr %6, align 2
  br label %7

7:                                                ; preds = %21, %2
  %8 = load i16, ptr %6, align 2
  %9 = load i16, ptr %4, align 2
  %10 = icmp slt i16 %8, %9
  br i1 %10, label %11, label %24

11:                                               ; preds = %7
  %12 = load i32, ptr %5, align 2
  %13 = mul nsw i32 %12, 3
  %14 = load ptr, ptr %3, align 2
  %15 = load i16, ptr %6, align 2
  %16 = getelementptr inbounds i16, ptr %14, i16 %15
  %17 = load i16, ptr %16, align 2
  %18 = sext i16 %17 to i32
  %19 = add nsw i32 %13, %18
  %20 = srem i32 %19, 1000003
  store i32 %20, ptr %5, align 2
  br label %21

21:                                               ; preds = %11
  %22 = load i16, ptr %6, align 2
  %23 = add nsw i16 %22, 1
  store i16 %23, ptr %6, align 2
  br label %7

24:                                               ; preds = %7
  %25 = load i32, ptr %5, align 2
  call void @llvm.lifetime.end.p0(ptr %6)
  call void @llvm.lifetime.end.p0(ptr %5)
  ret i32 %25
}

@a = global [48 x i16] zeroinitializer
@b = global [49 x i16] zeroinitializer
declare void @report(i32)
define void @callers() optsize {
  %x = call i32 @sum(ptr @a, i16 48)
  call void @report(i32 %x)
  %y = call i32 @sum(ptr @b, i16 49)
  call void @report(i32 %y)
  ret void
}
