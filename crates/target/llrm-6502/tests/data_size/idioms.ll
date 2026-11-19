target triple = "msp430"

@main.ns = internal constant [6 x i16] [i16 0, i16 1, i16 2, i16 17, i16 64, i16 100], align 2
@a = dso_local global [160 x i16] zeroinitializer, align 2
@pal = dso_local global [64 x i16] zeroinitializer, align 2
@c = dso_local global [160 x i16] zeroinitializer, align 2
@b = dso_local global [160 x i16] zeroinitializer, align 2
@L = dso_local global [40 x i32] zeroinitializer, align 2
@M = dso_local global [40 x i32] zeroinitializer, align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i16 0, ptr %2, align 2, !tbaa !7
  br label %5

5:                                                ; preds = %108, %0
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = icmp slt i16 %6, 6
  br i1 %7, label %8, label %111

8:                                                ; preds = %5
  %9 = load i16, ptr %2, align 2, !tbaa !7
  %10 = getelementptr inbounds [6 x i16], ptr @main.ns, i16 0, i16 %9
  %11 = load i16, ptr %10, align 2, !tbaa !7
  store i16 %11, ptr %4, align 2, !tbaa !7
  call void @init() #4
  %12 = load i16, ptr %4, align 2, !tbaa !7
  %13 = sdiv i16 %12, 10
  call void @clear_rows(i16 noundef signext %13) #4
  %14 = call i32 @sum(ptr noundef @a, i16 noundef signext 160) #4
  call void @report(i32 noundef %14) #4
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %21, %8
  %16 = load i16, ptr %3, align 2, !tbaa !7
  %17 = icmp slt i16 %16, 64
  br i1 %17, label %18, label %24

18:                                               ; preds = %15
  %19 = load i16, ptr %3, align 2, !tbaa !7
  %20 = getelementptr inbounds [64 x i16], ptr @pal, i16 0, i16 %19
  store i16 -1, ptr %20, align 2, !tbaa !7
  br label %21

21:                                               ; preds = %18
  %22 = load i16, ptr %3, align 2, !tbaa !7
  %23 = add nsw i16 %22, 1
  store i16 %23, ptr %3, align 2, !tbaa !7
  br label %15, !llvm.loop !9

24:                                               ; preds = %15
  %25 = load i16, ptr %4, align 2, !tbaa !7
  %26 = icmp slt i16 %25, 64
  br i1 %26, label %27, label %29

27:                                               ; preds = %24
  %28 = load i16, ptr %4, align 2, !tbaa !7
  br label %30

29:                                               ; preds = %24
  br label %30

30:                                               ; preds = %29, %27
  %31 = phi i16 [ %28, %27 ], [ 64, %29 ]
  %32 = load i16, ptr %4, align 2, !tbaa !7
  %33 = and i16 %32, 255
  %34 = shl i16 %33, 8
  %35 = or i16 %34, 52
  call void @palette(i16 noundef signext %31, i16 noundef signext %35) #4
  %36 = call i32 @sum(ptr noundef @pal, i16 noundef signext 64) #4
  call void @report(i32 noundef %36) #4
  call void @init() #4
  %37 = load i16, ptr %4, align 2, !tbaa !7
  call void @copy(i16 noundef signext %37) #4
  %38 = call i32 @sum(ptr noundef @a, i16 noundef signext 160) #4
  call void @report(i32 noundef %38) #4
  call void @init() #4
  %39 = load i16, ptr %4, align 2, !tbaa !7
  call void @scroll_up(i16 noundef signext %39) #4
  %40 = call i32 @sum(ptr noundef @a, i16 noundef signext 160) #4
  call void @report(i32 noundef %40) #4
  call void @init() #4
  %41 = load i16, ptr %4, align 2, !tbaa !7
  call void @scroll_down(i16 noundef signext %41) #4
  %42 = call i32 @sum(ptr noundef @a, i16 noundef signext 160) #4
  call void @report(i32 noundef %42) #4
  call void @init() #4
  %43 = load i16, ptr %4, align 2, !tbaa !7
  call void @smear(i16 noundef signext %43) #4
  %44 = call i32 @sum(ptr noundef @a, i16 noundef signext 160) #4
  call void @report(i32 noundef %44) #4
  call void @init() #4
  %45 = load i16, ptr %4, align 2, !tbaa !7
  call void @changed(i16 noundef signext %45) #4
  %46 = call i32 @sum(ptr noundef @a, i16 noundef signext 160) #4
  call void @report(i32 noundef %46) #4
  call void @init() #4
  %47 = load i16, ptr %4, align 2, !tbaa !7
  call void @two_stores(i16 noundef signext %47) #4
  %48 = call i32 @sum(ptr noundef @a, i16 noundef signext 160) #4
  %49 = call i32 @sum(ptr noundef @c, i16 noundef signext 160) #4
  %50 = add nsw i32 %48, %49
  call void @report(i32 noundef %50) #4
  call void @init() #4
  %51 = load i16, ptr %4, align 2, !tbaa !7
  call void @strides(i16 noundef signext %51) #4
  %52 = call i32 @sum(ptr noundef @a, i16 noundef signext 160) #4
  call void @report(i32 noundef %52) #4
  call void @init() #4
  %53 = load i16, ptr %4, align 2, !tbaa !7
  call void @written(i16 noundef signext %53) #4
  %54 = call i32 @sum(ptr noundef @a, i16 noundef signext 160) #4
  %55 = call i32 @sum(ptr noundef @b, i16 noundef signext 160) #4
  %56 = add nsw i32 %54, %55
  call void @report(i32 noundef %56) #4
  call void @init() #4
  %57 = load i16, ptr %4, align 2, !tbaa !7
  %58 = sdiv i16 %57, 10
  call void @block(i16 noundef signext %58) #4
  %59 = call i32 @sum(ptr noundef @a, i16 noundef signext 160) #4
  call void @report(i32 noundef %59) #4
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %60

60:                                               ; preds = %75, %30
  %61 = load i16, ptr %3, align 2, !tbaa !7
  %62 = icmp slt i16 %61, 40
  br i1 %62, label %63, label %78

63:                                               ; preds = %60
  %64 = load i16, ptr %3, align 2, !tbaa !7
  %65 = add nsw i16 4096, %64
  %66 = sext i16 %65 to i32
  %67 = load i16, ptr %3, align 2, !tbaa !7
  %68 = getelementptr inbounds [40 x i32], ptr @L, i16 0, i16 %67
  store i32 %66, ptr %68, align 2, !tbaa !11
  %69 = load i16, ptr %3, align 2, !tbaa !7
  %70 = mul nsw i16 3, %69
  %71 = add nsw i16 8192, %70
  %72 = sext i16 %71 to i32
  %73 = load i16, ptr %3, align 2, !tbaa !7
  %74 = getelementptr inbounds [40 x i32], ptr @M, i16 0, i16 %73
  store i32 %72, ptr %74, align 2, !tbaa !11
  br label %75

75:                                               ; preds = %63
  %76 = load i16, ptr %3, align 2, !tbaa !7
  %77 = add nsw i16 %76, 1
  store i16 %77, ptr %3, align 2, !tbaa !7
  br label %60, !llvm.loop !13

78:                                               ; preds = %60
  %79 = load i16, ptr %4, align 2, !tbaa !7
  %80 = icmp slt i16 %79, 40
  br i1 %80, label %81, label %83

81:                                               ; preds = %78
  %82 = load i16, ptr %4, align 2, !tbaa !7
  br label %84

83:                                               ; preds = %78
  br label %84

84:                                               ; preds = %83, %81
  %85 = phi i16 [ %82, %81 ], [ 40, %83 ]
  call void @copy_long(i16 noundef signext %85) #4
  %86 = call i32 @sumlong(ptr noundef @L, i16 noundef signext 40) #4
  call void @report(i32 noundef %86) #4
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %87

87:                                               ; preds = %93, %84
  %88 = load i16, ptr %3, align 2, !tbaa !7
  %89 = icmp slt i16 %88, 40
  br i1 %89, label %90, label %96

90:                                               ; preds = %87
  %91 = load i16, ptr %3, align 2, !tbaa !7
  %92 = getelementptr inbounds [40 x i32], ptr @L, i16 0, i16 %91
  store i32 -1, ptr %92, align 2, !tbaa !11
  br label %93

93:                                               ; preds = %90
  %94 = load i16, ptr %3, align 2, !tbaa !7
  %95 = add nsw i16 %94, 1
  store i16 %95, ptr %3, align 2, !tbaa !7
  br label %87, !llvm.loop !14

96:                                               ; preds = %87
  %97 = load i16, ptr %4, align 2, !tbaa !7
  %98 = icmp slt i16 %97, 40
  br i1 %98, label %99, label %101

99:                                               ; preds = %96
  %100 = load i16, ptr %4, align 2, !tbaa !7
  br label %102

101:                                              ; preds = %96
  br label %102

102:                                              ; preds = %101, %99
  %103 = phi i16 [ %100, %99 ], [ 40, %101 ]
  %104 = load i16, ptr %4, align 2, !tbaa !7
  %105 = sext i16 %104 to i32
  %106 = add nsw i32 1193046, %105
  call void @fill_long(i16 noundef signext %103, i32 noundef %106) #4
  %107 = call i32 @sumlong(ptr noundef @L, i16 noundef signext 40) #4
  call void @report(i32 noundef %107) #4
  br label %108

108:                                              ; preds = %102
  %109 = load i16, ptr %2, align 2, !tbaa !7
  %110 = add nsw i16 %109, 1
  store i16 %110, ptr %2, align 2, !tbaa !7
  br label %5, !llvm.loop !15

111:                                              ; preds = %5
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal void @init() #0 {
  %1 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store i16 0, ptr %1, align 2, !tbaa !7
  br label %2

2:                                                ; preds = %20, %0
  %3 = load i16, ptr %1, align 2, !tbaa !7
  %4 = icmp slt i16 %3, 160
  br i1 %4, label %5, label %23

5:                                                ; preds = %2
  %6 = load i16, ptr %1, align 2, !tbaa !7
  %7 = add nsw i16 %6, 1000
  %8 = load i16, ptr %1, align 2, !tbaa !7
  %9 = getelementptr inbounds [160 x i16], ptr @a, i16 0, i16 %8
  store i16 %7, ptr %9, align 2, !tbaa !7
  %10 = load i16, ptr %1, align 2, !tbaa !7
  %11 = mul nsw i16 %10, 3
  %12 = add nsw i16 %11, 1
  %13 = load i16, ptr %1, align 2, !tbaa !7
  %14 = getelementptr inbounds [160 x i16], ptr @b, i16 0, i16 %13
  store i16 %12, ptr %14, align 2, !tbaa !7
  %15 = load i16, ptr %1, align 2, !tbaa !7
  %16 = mul nsw i16 %15, 5
  %17 = add nsw i16 %16, 2
  %18 = load i16, ptr %1, align 2, !tbaa !7
  %19 = getelementptr inbounds [160 x i16], ptr @c, i16 0, i16 %18
  store i16 %17, ptr %19, align 2, !tbaa !7
  br label %20

20:                                               ; preds = %5
  %21 = load i16, ptr %1, align 2, !tbaa !7
  %22 = add nsw i16 %21, 1
  store i16 %22, ptr %1, align 2, !tbaa !7
  br label %2, !llvm.loop !16

23:                                               ; preds = %2
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret void
}

; Function Attrs: nounwind optsize
define internal void @clear_rows(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %5

5:                                                ; preds = %23, %1
  %6 = load i16, ptr %3, align 2, !tbaa !7
  %7 = load i16, ptr %2, align 2, !tbaa !7
  %8 = icmp slt i16 %6, %7
  br i1 %8, label %9, label %26

9:                                                ; preds = %5
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %19, %9
  %11 = load i16, ptr %4, align 2, !tbaa !7
  %12 = icmp slt i16 %11, 16
  br i1 %12, label %13, label %22

13:                                               ; preds = %10
  %14 = load i16, ptr %3, align 2, !tbaa !7
  %15 = mul nsw i16 %14, 16
  %16 = load i16, ptr %4, align 2, !tbaa !7
  %17 = add nsw i16 %15, %16
  %18 = getelementptr inbounds [160 x i16], ptr @a, i16 0, i16 %17
  store i16 4660, ptr %18, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %13
  %20 = load i16, ptr %4, align 2, !tbaa !7
  %21 = add nsw i16 %20, 1
  store i16 %21, ptr %4, align 2, !tbaa !7
  br label %10, !llvm.loop !17

22:                                               ; preds = %10
  br label %23

23:                                               ; preds = %22
  %24 = load i16, ptr %3, align 2, !tbaa !7
  %25 = add nsw i16 %24, 1
  store i16 %25, ptr %3, align 2, !tbaa !7
  br label %5, !llvm.loop !18

26:                                               ; preds = %5
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nounwind optsize
define internal i32 @sum(ptr noundef %0, i16 noundef signext %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i16, align 2
  store ptr %0, ptr %3, align 2, !tbaa !19
  store i16 %1, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !11
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i16 0, ptr %6, align 2, !tbaa !7
  br label %7

7:                                                ; preds = %23, %2
  %8 = load i16, ptr %6, align 2, !tbaa !7
  %9 = load i16, ptr %4, align 2, !tbaa !7
  %10 = icmp slt i16 %8, %9
  br i1 %10, label %11, label %26

11:                                               ; preds = %7
  %12 = load i16, ptr %6, align 2, !tbaa !7
  %13 = add nsw i16 %12, 1
  %14 = sext i16 %13 to i32
  %15 = load ptr, ptr %3, align 2, !tbaa !19
  %16 = load i16, ptr %6, align 2, !tbaa !7
  %17 = getelementptr inbounds i16, ptr %15, i16 %16
  %18 = load i16, ptr %17, align 2, !tbaa !7
  %19 = sext i16 %18 to i32
  %20 = mul nsw i32 %14, %19
  %21 = load i32, ptr %5, align 2, !tbaa !11
  %22 = add nsw i32 %21, %20
  store i32 %22, ptr %5, align 2, !tbaa !11
  br label %23

23:                                               ; preds = %11
  %24 = load i16, ptr %6, align 2, !tbaa !7
  %25 = add nsw i16 %24, 1
  store i16 %25, ptr %6, align 2, !tbaa !7
  br label %7, !llvm.loop !22

26:                                               ; preds = %7
  %27 = load i32, ptr %5, align 2, !tbaa !11
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret i32 %27
}

; Function Attrs: nounwind optsize
define internal void @palette(i16 noundef signext %0, i16 noundef signext %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  store i16 %1, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 0, ptr %5, align 2, !tbaa !7
  br label %6

6:                                                ; preds = %14, %2
  %7 = load i16, ptr %5, align 2, !tbaa !7
  %8 = load i16, ptr %3, align 2, !tbaa !7
  %9 = icmp slt i16 %7, %8
  br i1 %9, label %10, label %17

10:                                               ; preds = %6
  %11 = load i16, ptr %4, align 2, !tbaa !7
  %12 = load i16, ptr %5, align 2, !tbaa !7
  %13 = getelementptr inbounds [64 x i16], ptr @pal, i16 0, i16 %12
  store i16 %11, ptr %13, align 2, !tbaa !7
  br label %14

14:                                               ; preds = %10
  %15 = load i16, ptr %5, align 2, !tbaa !7
  %16 = add nsw i16 %15, 1
  store i16 %16, ptr %5, align 2, !tbaa !7
  br label %6, !llvm.loop !23

17:                                               ; preds = %6
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret void
}

; Function Attrs: nounwind optsize
define internal void @copy(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %14, %1
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = icmp slt i16 %5, %6
  br i1 %7, label %8, label %17

8:                                                ; preds = %4
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = getelementptr inbounds [160 x i16], ptr @b, i16 0, i16 %9
  %11 = load i16, ptr %10, align 2, !tbaa !7
  %12 = load i16, ptr %3, align 2, !tbaa !7
  %13 = getelementptr inbounds [160 x i16], ptr @a, i16 0, i16 %12
  store i16 %11, ptr %13, align 2, !tbaa !7
  br label %14

14:                                               ; preds = %8
  %15 = load i16, ptr %3, align 2, !tbaa !7
  %16 = add nsw i16 %15, 1
  store i16 %16, ptr %3, align 2, !tbaa !7
  br label %4, !llvm.loop !24

17:                                               ; preds = %4
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

; Function Attrs: nounwind optsize
define internal void @scroll_up(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %1
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = icmp slt i16 %5, %6
  br i1 %7, label %8, label %18

8:                                                ; preds = %4
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = add nsw i16 %9, 16
  %11 = getelementptr inbounds [160 x i16], ptr @a, i16 0, i16 %10
  %12 = load i16, ptr %11, align 2, !tbaa !7
  %13 = load i16, ptr %3, align 2, !tbaa !7
  %14 = getelementptr inbounds [160 x i16], ptr @a, i16 0, i16 %13
  store i16 %12, ptr %14, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %8
  %16 = load i16, ptr %3, align 2, !tbaa !7
  %17 = add nsw i16 %16, 1
  store i16 %17, ptr %3, align 2, !tbaa !7
  br label %4, !llvm.loop !25

18:                                               ; preds = %4
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

; Function Attrs: nounwind optsize
define internal void @scroll_down(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  %4 = load i16, ptr %2, align 2, !tbaa !7
  %5 = sub nsw i16 %4, 1
  store i16 %5, ptr %3, align 2, !tbaa !7
  br label %6

6:                                                ; preds = %16, %1
  %7 = load i16, ptr %3, align 2, !tbaa !7
  %8 = icmp sge i16 %7, 0
  br i1 %8, label %9, label %19

9:                                                ; preds = %6
  %10 = load i16, ptr %3, align 2, !tbaa !7
  %11 = getelementptr inbounds [160 x i16], ptr @a, i16 0, i16 %10
  %12 = load i16, ptr %11, align 2, !tbaa !7
  %13 = load i16, ptr %3, align 2, !tbaa !7
  %14 = add nsw i16 %13, 16
  %15 = getelementptr inbounds [160 x i16], ptr @a, i16 0, i16 %14
  store i16 %12, ptr %15, align 2, !tbaa !7
  br label %16

16:                                               ; preds = %9
  %17 = load i16, ptr %3, align 2, !tbaa !7
  %18 = add nsw i16 %17, -1
  store i16 %18, ptr %3, align 2, !tbaa !7
  br label %6, !llvm.loop !26

19:                                               ; preds = %6
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

; Function Attrs: nounwind optsize
define internal void @smear(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %1
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = icmp slt i16 %5, %6
  br i1 %7, label %8, label %18

8:                                                ; preds = %4
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = getelementptr inbounds [160 x i16], ptr @a, i16 0, i16 %9
  %11 = load i16, ptr %10, align 2, !tbaa !7
  %12 = load i16, ptr %3, align 2, !tbaa !7
  %13 = add nsw i16 %12, 1
  %14 = getelementptr inbounds [160 x i16], ptr @a, i16 0, i16 %13
  store i16 %11, ptr %14, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %8
  %16 = load i16, ptr %3, align 2, !tbaa !7
  %17 = add nsw i16 %16, 1
  store i16 %17, ptr %3, align 2, !tbaa !7
  br label %4, !llvm.loop !27

18:                                               ; preds = %4
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

; Function Attrs: nounwind optsize
define internal void @changed(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %1
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = icmp slt i16 %5, %6
  br i1 %7, label %8, label %18

8:                                                ; preds = %4
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = getelementptr inbounds [160 x i16], ptr @b, i16 0, i16 %9
  %11 = load i16, ptr %10, align 2, !tbaa !7
  %12 = add nsw i16 %11, 1
  %13 = load i16, ptr %3, align 2, !tbaa !7
  %14 = getelementptr inbounds [160 x i16], ptr @a, i16 0, i16 %13
  store i16 %12, ptr %14, align 2, !tbaa !7
  br label %15

15:                                               ; preds = %8
  %16 = load i16, ptr %3, align 2, !tbaa !7
  %17 = add nsw i16 %16, 1
  store i16 %17, ptr %3, align 2, !tbaa !7
  br label %4, !llvm.loop !28

18:                                               ; preds = %4
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

; Function Attrs: nounwind optsize
define internal void @two_stores(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %16, %1
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = icmp slt i16 %5, %6
  br i1 %7, label %8, label %19

8:                                                ; preds = %4
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = getelementptr inbounds [160 x i16], ptr @b, i16 0, i16 %9
  %11 = load i16, ptr %10, align 2, !tbaa !7
  %12 = load i16, ptr %3, align 2, !tbaa !7
  %13 = getelementptr inbounds [160 x i16], ptr @a, i16 0, i16 %12
  store i16 %11, ptr %13, align 2, !tbaa !7
  %14 = load i16, ptr %3, align 2, !tbaa !7
  %15 = getelementptr inbounds [160 x i16], ptr @c, i16 0, i16 %14
  store i16 0, ptr %15, align 2, !tbaa !7
  br label %16

16:                                               ; preds = %8
  %17 = load i16, ptr %3, align 2, !tbaa !7
  %18 = add nsw i16 %17, 1
  store i16 %18, ptr %3, align 2, !tbaa !7
  br label %4, !llvm.loop !29

19:                                               ; preds = %4
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

; Function Attrs: nounwind optsize
define internal void @strides(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %16, %1
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = sdiv i16 %6, 2
  %8 = icmp slt i16 %5, %7
  br i1 %8, label %9, label %19

9:                                                ; preds = %4
  %10 = load i16, ptr %3, align 2, !tbaa !7
  %11 = getelementptr inbounds [160 x i16], ptr @b, i16 0, i16 %10
  %12 = load i16, ptr %11, align 2, !tbaa !7
  %13 = load i16, ptr %3, align 2, !tbaa !7
  %14 = mul nsw i16 2, %13
  %15 = getelementptr inbounds [160 x i16], ptr @a, i16 0, i16 %14
  store i16 %12, ptr %15, align 2, !tbaa !7
  br label %16

16:                                               ; preds = %9
  %17 = load i16, ptr %3, align 2, !tbaa !7
  %18 = add nsw i16 %17, 1
  store i16 %18, ptr %3, align 2, !tbaa !7
  br label %4, !llvm.loop !30

19:                                               ; preds = %4
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

; Function Attrs: nounwind optsize
define internal void @written(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %16, %1
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = icmp slt i16 %5, %6
  br i1 %7, label %8, label %19

8:                                                ; preds = %4
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = getelementptr inbounds [160 x i16], ptr @b, i16 0, i16 %9
  store i16 0, ptr %10, align 2, !tbaa !7
  %11 = load i16, ptr %3, align 2, !tbaa !7
  %12 = getelementptr inbounds [160 x i16], ptr @b, i16 0, i16 %11
  %13 = load i16, ptr %12, align 2, !tbaa !7
  %14 = load i16, ptr %3, align 2, !tbaa !7
  %15 = getelementptr inbounds [160 x i16], ptr @a, i16 0, i16 %14
  store i16 %13, ptr %15, align 2, !tbaa !7
  br label %16

16:                                               ; preds = %8
  %17 = load i16, ptr %3, align 2, !tbaa !7
  %18 = add nsw i16 %17, 1
  store i16 %18, ptr %3, align 2, !tbaa !7
  br label %4, !llvm.loop !31

19:                                               ; preds = %4
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

; Function Attrs: nounwind optsize
define internal void @block(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %5

5:                                                ; preds = %29, %1
  %6 = load i16, ptr %3, align 2, !tbaa !7
  %7 = icmp slt i16 %6, 4
  br i1 %7, label %8, label %32

8:                                                ; preds = %5
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %9

9:                                                ; preds = %25, %8
  %10 = load i16, ptr %4, align 2, !tbaa !7
  %11 = load i16, ptr %2, align 2, !tbaa !7
  %12 = icmp slt i16 %10, %11
  br i1 %12, label %13, label %28

13:                                               ; preds = %9
  %14 = load i16, ptr %3, align 2, !tbaa !7
  %15 = mul nsw i16 %14, 20
  %16 = load i16, ptr %4, align 2, !tbaa !7
  %17 = add nsw i16 %15, %16
  %18 = getelementptr inbounds [160 x i16], ptr @b, i16 0, i16 %17
  %19 = load i16, ptr %18, align 2, !tbaa !7
  %20 = load i16, ptr %3, align 2, !tbaa !7
  %21 = mul nsw i16 %20, 16
  %22 = load i16, ptr %4, align 2, !tbaa !7
  %23 = add nsw i16 %21, %22
  %24 = getelementptr inbounds [160 x i16], ptr @a, i16 0, i16 %23
  store i16 %19, ptr %24, align 2, !tbaa !7
  br label %25

25:                                               ; preds = %13
  %26 = load i16, ptr %4, align 2, !tbaa !7
  %27 = add nsw i16 %26, 1
  store i16 %27, ptr %4, align 2, !tbaa !7
  br label %9, !llvm.loop !32

28:                                               ; preds = %9
  br label %29

29:                                               ; preds = %28
  %30 = load i16, ptr %3, align 2, !tbaa !7
  %31 = add nsw i16 %30, 1
  store i16 %31, ptr %3, align 2, !tbaa !7
  br label %5, !llvm.loop !33

32:                                               ; preds = %5
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

; Function Attrs: nounwind optsize
define internal void @copy_long(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %14, %1
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = icmp slt i16 %5, %6
  br i1 %7, label %8, label %17

8:                                                ; preds = %4
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = getelementptr inbounds [40 x i32], ptr @M, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2, !tbaa !11
  %12 = load i16, ptr %3, align 2, !tbaa !7
  %13 = getelementptr inbounds [40 x i32], ptr @L, i16 0, i16 %12
  store i32 %11, ptr %13, align 2, !tbaa !11
  br label %14

14:                                               ; preds = %8
  %15 = load i16, ptr %3, align 2, !tbaa !7
  %16 = add nsw i16 %15, 1
  store i16 %16, ptr %3, align 2, !tbaa !7
  br label %4, !llvm.loop !34

17:                                               ; preds = %4
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

; Function Attrs: nounwind optsize
define internal i32 @sumlong(ptr noundef %0, i16 noundef signext %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i16, align 2
  store ptr %0, ptr %3, align 2, !tbaa !35
  store i16 %1, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !11
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i16 0, ptr %6, align 2, !tbaa !7
  br label %7

7:                                                ; preds = %22, %2
  %8 = load i16, ptr %6, align 2, !tbaa !7
  %9 = load i16, ptr %4, align 2, !tbaa !7
  %10 = icmp slt i16 %8, %9
  br i1 %10, label %11, label %25

11:                                               ; preds = %7
  %12 = load i16, ptr %6, align 2, !tbaa !7
  %13 = add nsw i16 %12, 1
  %14 = sext i16 %13 to i32
  %15 = load ptr, ptr %3, align 2, !tbaa !35
  %16 = load i16, ptr %6, align 2, !tbaa !7
  %17 = getelementptr inbounds i32, ptr %15, i16 %16
  %18 = load i32, ptr %17, align 2, !tbaa !11
  %19 = mul nsw i32 %14, %18
  %20 = load i32, ptr %5, align 2, !tbaa !11
  %21 = add nsw i32 %20, %19
  store i32 %21, ptr %5, align 2, !tbaa !11
  br label %22

22:                                               ; preds = %11
  %23 = load i16, ptr %6, align 2, !tbaa !7
  %24 = add nsw i16 %23, 1
  store i16 %24, ptr %6, align 2, !tbaa !7
  br label %7, !llvm.loop !37

25:                                               ; preds = %7
  %26 = load i32, ptr %5, align 2, !tbaa !11
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret i32 %26
}

; Function Attrs: nounwind optsize
define internal void @fill_long(i16 noundef signext %0, i32 noundef %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i32, align 2
  %5 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  store i32 %1, ptr %4, align 2, !tbaa !11
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 0, ptr %5, align 2, !tbaa !7
  br label %6

6:                                                ; preds = %14, %2
  %7 = load i16, ptr %5, align 2, !tbaa !7
  %8 = load i16, ptr %3, align 2, !tbaa !7
  %9 = icmp slt i16 %7, %8
  br i1 %9, label %10, label %17

10:                                               ; preds = %6
  %11 = load i32, ptr %4, align 2, !tbaa !11
  %12 = load i16, ptr %5, align 2, !tbaa !7
  %13 = getelementptr inbounds [40 x i32], ptr @L, i16 0, i16 %12
  store i32 %11, ptr %13, align 2, !tbaa !11
  br label %14

14:                                               ; preds = %10
  %15 = load i16, ptr %5, align 2, !tbaa !7
  %16 = add nsw i16 %15, 1
  store i16 %16, ptr %5, align 2, !tbaa !7
  br label %6, !llvm.loop !38

17:                                               ; preds = %6
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }
attributes #4 = { optsize }

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
!8 = !{!"short", !5, i64 0}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
!11 = !{!12, !12, i64 0}
!12 = !{!"long", !5, i64 0}
!13 = distinct !{!13, !10}
!14 = distinct !{!14, !10}
!15 = distinct !{!15, !10}
!16 = distinct !{!16, !10}
!17 = distinct !{!17, !10}
!18 = distinct !{!18, !10}
!19 = !{!20, !20, i64 0}
!20 = !{!"p1 short", !21, i64 0}
!21 = !{!"any pointer", !5, i64 0}
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
!35 = !{!36, !36, i64 0}
!36 = !{!"p1 long", !21, i64 0}
!37 = distinct !{!37, !10}
!38 = distinct !{!38, !10}
