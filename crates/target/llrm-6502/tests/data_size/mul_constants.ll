target triple = "msp430"

@values = internal global [8 x i32] [i32 0, i32 1, i32 2, i32 32767, i32 32768, i32 -1, i32 -2147483648, i32 305419897], align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i32 @sum3UL() #3
  call void @report(i32 noundef %2) #3
  %3 = call i32 @sum5UL() #3
  call void @report(i32 noundef %3) #3
  %4 = call i32 @sum9UL() #3
  call void @report(i32 noundef %4) #3
  %5 = call i32 @sum6UL() #3
  call void @report(i32 noundef %5) #3
  %6 = call i32 @sum10UL() #3
  call void @report(i32 noundef %6) #3
  %7 = call i32 @sum12UL() #3
  call void @report(i32 noundef %7) #3
  %8 = call i32 @sum17UL() #3
  call void @report(i32 noundef %8) #3
  %9 = call i32 @sum18UL() #3
  call void @report(i32 noundef %9) #3
  %10 = call i32 @sum20UL() #3
  call void @report(i32 noundef %10) #3
  %11 = call i32 @sum24UL() #3
  call void @report(i32 noundef %11) #3
  %12 = call i32 @sum25UL() #3
  call void @report(i32 noundef %12) #3
  %13 = call i32 @sum36UL() #3
  call void @report(i32 noundef %13) #3
  %14 = call i32 @sum40UL() #3
  call void @report(i32 noundef %14) #3
  %15 = call i32 @sum45UL() #3
  call void @report(i32 noundef %15) #3
  %16 = call i32 @sum72UL() #3
  call void @report(i32 noundef %16) #3
  %17 = call i32 @sum81UL() #3
  call void @report(i32 noundef %17) #3
  %18 = call i32 @sum100UL() #3
  call void @report(i32 noundef %18) #3
  %19 = call i32 @sum125UL() #3
  call void @report(i32 noundef %19) #3
  %20 = call i32 @sum127UL() #3
  call void @report(i32 noundef %20) #3
  %21 = call i32 @sum129UL() #3
  call void @report(i32 noundef %21) #3
  %22 = call i32 @sum255UL() #3
  call void @report(i32 noundef %22) #3
  %23 = call i32 @sum257UL() #3
  call void @report(i32 noundef %23) #3
  %24 = call i32 @sum1000UL() #3
  call void @report(i32 noundef %24) #3
  %25 = call i32 @sum65537UL() #3
  call void @report(i32 noundef %25) #3
  %26 = call i32 @sum7UL() #3
  call void @report(i32 noundef %26) #3
  %27 = call i32 @sum14UL() #3
  call void @report(i32 noundef %27) #3
  %28 = call i32 @sum15UL() #3
  call void @report(i32 noundef %28) #3
  %29 = call i32 @sum28UL() #3
  call void @report(i32 noundef %29) #3
  %30 = call i32 @sum30UL() #3
  call void @report(i32 noundef %30) #3
  %31 = call i32 @sum31UL() #3
  call void @report(i32 noundef %31) #3
  %32 = call i32 @sum62UL() #3
  call void @report(i32 noundef %32) #3
  %33 = call i32 @sum63UL() #3
  call void @report(i32 noundef %33) #3
  ret i16 0
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #1

; Function Attrs: nounwind optsize
define internal i32 @sum3UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 3
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !9

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 3
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !11

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum5UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 5
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !12

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 5
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !13

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum9UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 9
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !14

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 9
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !15

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum6UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 6
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !16

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 6
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !17

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum10UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 10
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !18

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 10
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !19

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum12UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 12
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !20

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 12
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !21

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum17UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 17
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !22

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 17
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !23

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum18UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 18
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !24

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 18
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !25

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum20UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 20
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !26

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 20
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !27

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum24UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 24
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !28

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 24
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !29

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum25UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 25
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !30

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 25
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !31

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum36UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 36
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !32

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 36
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !33

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum40UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 40
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !34

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 40
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !35

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum45UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 45
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !36

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 45
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !37

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum72UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 72
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !38

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 72
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !39

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum81UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 81
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !40

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 81
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !41

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum100UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 100
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !42

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 100
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !43

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum125UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 125
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !44

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 125
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !45

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum127UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 127
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !46

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 127
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !47

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum129UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 129
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !48

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 129
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !49

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum255UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 255
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !50

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 255
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !51

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum257UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 257
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !52

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 257
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !53

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum1000UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 1000
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !54

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 1000
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !55

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum65537UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 65537
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !56

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 65537
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !57

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum7UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 7
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !58

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 7
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !59

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum14UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 14
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !60

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 14
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !61

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum15UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 15
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !62

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 15
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !63

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum28UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 28
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !64

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 28
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !65

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum30UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 30
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !66

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 30
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !67

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum31UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 31
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !68

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 31
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !69

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum62UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 62
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !70

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 62
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !71

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nounwind optsize
define internal i32 @sum63UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i32 0, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 -1640531527, ptr %3, align 2, !tbaa !7
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !7
  %12 = mul i32 %11, 63
  %13 = load i32, ptr %1, align 2, !tbaa !7
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !72

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2, !tbaa !7
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2, !tbaa !7
  %26 = load i32, ptr %1, align 2, !tbaa !7
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = mul i32 %28, 63
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2, !tbaa !7
  br label %19, !llvm.loop !73

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %35
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #2

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { optsize }
attributes #4 = { nounwind }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!8, !8, i64 0}
!8 = !{!"long", !5, i64 0}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
!11 = distinct !{!11, !10}
!12 = distinct !{!12, !10}
!13 = distinct !{!13, !10}
!14 = distinct !{!14, !10}
!15 = distinct !{!15, !10}
!16 = distinct !{!16, !10}
!17 = distinct !{!17, !10}
!18 = distinct !{!18, !10}
!19 = distinct !{!19, !10}
!20 = distinct !{!20, !10}
!21 = distinct !{!21, !10}
!22 = distinct !{!22, !10}
!23 = distinct !{!23, !10}
!24 = distinct !{!24, !10}
!25 = distinct !{!25, !10}
!26 = distinct !{!26, !10}
!27 = distinct !{!27, !10}
!28 = distinct !{!28, !10}
!29 = distinct !{!29, !10}
!30 = distinct !{!30, !10}
!31 = distinct !{!31, !10}
!32 = distinct !{!32, !10}
!33 = distinct !{!33, !10}
!34 = distinct !{!34, !10}
!35 = distinct !{!35, !10}
!36 = distinct !{!36, !10}
!37 = distinct !{!37, !10}
!38 = distinct !{!38, !10}
!39 = distinct !{!39, !10}
!40 = distinct !{!40, !10}
!41 = distinct !{!41, !10}
!42 = distinct !{!42, !10}
!43 = distinct !{!43, !10}
!44 = distinct !{!44, !10}
!45 = distinct !{!45, !10}
!46 = distinct !{!46, !10}
!47 = distinct !{!47, !10}
!48 = distinct !{!48, !10}
!49 = distinct !{!49, !10}
!50 = distinct !{!50, !10}
!51 = distinct !{!51, !10}
!52 = distinct !{!52, !10}
!53 = distinct !{!53, !10}
!54 = distinct !{!54, !10}
!55 = distinct !{!55, !10}
!56 = distinct !{!56, !10}
!57 = distinct !{!57, !10}
!58 = distinct !{!58, !10}
!59 = distinct !{!59, !10}
!60 = distinct !{!60, !10}
!61 = distinct !{!61, !10}
!62 = distinct !{!62, !10}
!63 = distinct !{!63, !10}
!64 = distinct !{!64, !10}
!65 = distinct !{!65, !10}
!66 = distinct !{!66, !10}
!67 = distinct !{!67, !10}
!68 = distinct !{!68, !10}
!69 = distinct !{!69, !10}
!70 = distinct !{!70, !10}
!71 = distinct !{!71, !10}
!72 = distinct !{!72, !10}
!73 = distinct !{!73, !10}
