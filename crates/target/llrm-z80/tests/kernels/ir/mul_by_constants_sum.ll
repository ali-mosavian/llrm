target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
%struct.tiny = type { i8, i8, i8 }
define i16 @three(i16 noundef %0, ptr noundef byval(%struct.tiny) align 1 %1, ptr noundef byval(%struct.tiny) align 1 %2, i32 noundef %3) {
  %5 = alloca i16, align 2
  %6 = alloca i32, align 2
  store i16 %0, ptr %5, align 2
  store i32 %3, ptr %6, align 2
  %7 = load i16, ptr %5, align 2
  %8 = getelementptr inbounds nuw %struct.tiny, ptr %1, i32 0, i32 0
  %9 = load i8, ptr %8, align 1
  %10 = sext i8 %9 to i16
  %11 = mul nsw i16 %10, 10
  %12 = add nsw i16 %7, %11
  %13 = getelementptr inbounds nuw %struct.tiny, ptr %1, i32 0, i32 1
  %14 = load i8, ptr %13, align 1
  %15 = sext i8 %14 to i16
  %16 = mul nsw i16 %15, 100
  %17 = add nsw i16 %12, %16
  %18 = getelementptr inbounds nuw %struct.tiny, ptr %1, i32 0, i32 2
  %19 = load i8, ptr %18, align 1
  %20 = sext i8 %19 to i16
  %21 = add nsw i16 %17, %20
  %22 = getelementptr inbounds nuw %struct.tiny, ptr %2, i32 0, i32 0
  %23 = load i8, ptr %22, align 1
  %24 = sext i8 %23 to i16
  %25 = mul nsw i16 %24, 3
  %26 = add nsw i16 %21, %25
  %27 = getelementptr inbounds nuw %struct.tiny, ptr %2, i32 0, i32 1
  %28 = load i8, ptr %27, align 1
  %29 = sext i8 %28 to i16
  %30 = mul nsw i16 %29, 5
  %31 = add nsw i16 %26, %30
  %32 = getelementptr inbounds nuw %struct.tiny, ptr %2, i32 0, i32 2
  %33 = load i8, ptr %32, align 1
  %34 = sext i8 %33 to i16
  %35 = mul nsw i16 %34, 7
  %36 = add nsw i16 %31, %35
  %37 = load i32, ptr %6, align 2
  %38 = trunc i32 %37 to i16
  %39 = add nsw i16 %36, %38
  ret i16 %39
}
