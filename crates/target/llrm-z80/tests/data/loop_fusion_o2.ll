; ModuleID = 'tests/run/c/loop_fusion.c'
source_filename = "tests/run/c/loop_fusion.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
target triple = "msp430"

@a = internal global [64 x i16] zeroinitializer, align 2
@b = internal global [64 x i16] zeroinitializer, align 2

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i32 @same(i16 noundef signext 10)
  call void @report(i32 noundef %2)
  %3 = call i32 @same(i16 noundef signext 0)
  call void @report(i32 noundef %3)
  %4 = call i32 @same(i16 noundef signext 1)
  call void @report(i32 noundef %4)
  %5 = call i32 @behind(i16 noundef signext 10)
  call void @report(i32 noundef %5)
  %6 = call i32 @behind(i16 noundef signext 0)
  call void @report(i32 noundef %6)
  %7 = call i32 @ahead(i16 noundef signext 10)
  call void @report(i32 noundef %7)
  %8 = call i32 @ahead(i16 noundef signext 1)
  call void @report(i32 noundef %8)
  %9 = call i32 @over(i16 noundef signext 10)
  call void @report(i32 noundef %9)
  %10 = call i32 @over(i16 noundef signext 0)
  call void @report(i32 noundef %10)
  %11 = call i32 @longer(i16 noundef signext 10)
  call void @report(i32 noundef %11)
  %12 = call i32 @longer(i16 noundef signext 0)
  call void @report(i32 noundef %12)
  ret i16 0
}

declare dso_local void @report(i32 noundef) #1

; Function Attrs: nounwind
define internal i32 @same(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @clear()
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %13, %1
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = icmp slt i16 %5, %6
  br i1 %7, label %8, label %16

8:                                                ; preds = %4
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = mul nsw i16 %9, 3
  %11 = load i16, ptr %3, align 2, !tbaa !7
  %12 = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %11
  store i16 %10, ptr %12, align 2, !tbaa !7
  br label %13

13:                                               ; preds = %8
  %14 = load i16, ptr %3, align 2, !tbaa !7
  %15 = add nsw i16 %14, 1
  store i16 %15, ptr %3, align 2, !tbaa !7
  br label %4, !llvm.loop !9

16:                                               ; preds = %4
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %17

17:                                               ; preds = %28, %16
  %18 = load i16, ptr %3, align 2, !tbaa !7
  %19 = load i16, ptr %2, align 2, !tbaa !7
  %20 = icmp slt i16 %18, %19
  br i1 %20, label %21, label %31

21:                                               ; preds = %17
  %22 = load i16, ptr %3, align 2, !tbaa !7
  %23 = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %22
  %24 = load i16, ptr %23, align 2, !tbaa !7
  %25 = add nsw i16 %24, 1
  %26 = load i16, ptr %3, align 2, !tbaa !7
  %27 = getelementptr inbounds [64 x i16], ptr @b, i16 0, i16 %26
  store i16 %25, ptr %27, align 2, !tbaa !7
  br label %28

28:                                               ; preds = %21
  %29 = load i16, ptr %3, align 2, !tbaa !7
  %30 = add nsw i16 %29, 1
  store i16 %30, ptr %3, align 2, !tbaa !7
  br label %17, !llvm.loop !11

31:                                               ; preds = %17
  %32 = load i16, ptr %2, align 2, !tbaa !7
  %33 = call i32 @total(i16 noundef signext %32)
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %33
}

; Function Attrs: nounwind
define internal i32 @behind(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @clear()
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %14, %1
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = icmp slt i16 %5, %6
  br i1 %7, label %8, label %17

8:                                                ; preds = %4
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = mul nsw i16 %9, 3
  %11 = load i16, ptr %3, align 2, !tbaa !7
  %12 = add nsw i16 %11, 1
  %13 = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %12
  store i16 %10, ptr %13, align 2, !tbaa !7
  br label %14

14:                                               ; preds = %8
  %15 = load i16, ptr %3, align 2, !tbaa !7
  %16 = add nsw i16 %15, 1
  store i16 %16, ptr %3, align 2, !tbaa !7
  br label %4, !llvm.loop !12

17:                                               ; preds = %4
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %18

18:                                               ; preds = %29, %17
  %19 = load i16, ptr %3, align 2, !tbaa !7
  %20 = load i16, ptr %2, align 2, !tbaa !7
  %21 = icmp slt i16 %19, %20
  br i1 %21, label %22, label %32

22:                                               ; preds = %18
  %23 = load i16, ptr %3, align 2, !tbaa !7
  %24 = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %23
  %25 = load i16, ptr %24, align 2, !tbaa !7
  %26 = add nsw i16 %25, 1
  %27 = load i16, ptr %3, align 2, !tbaa !7
  %28 = getelementptr inbounds [64 x i16], ptr @b, i16 0, i16 %27
  store i16 %26, ptr %28, align 2, !tbaa !7
  br label %29

29:                                               ; preds = %22
  %30 = load i16, ptr %3, align 2, !tbaa !7
  %31 = add nsw i16 %30, 1
  store i16 %31, ptr %3, align 2, !tbaa !7
  br label %18, !llvm.loop !13

32:                                               ; preds = %18
  %33 = load i16, ptr %2, align 2, !tbaa !7
  %34 = call i32 @total(i16 noundef signext %33)
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %34
}

; Function Attrs: nounwind
define internal i32 @ahead(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @clear()
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %13, %1
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = icmp slt i16 %5, %6
  br i1 %7, label %8, label %16

8:                                                ; preds = %4
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = mul nsw i16 %9, 3
  %11 = load i16, ptr %3, align 2, !tbaa !7
  %12 = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %11
  store i16 %10, ptr %12, align 2, !tbaa !7
  br label %13

13:                                               ; preds = %8
  %14 = load i16, ptr %3, align 2, !tbaa !7
  %15 = add nsw i16 %14, 1
  store i16 %15, ptr %3, align 2, !tbaa !7
  br label %4, !llvm.loop !14

16:                                               ; preds = %4
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %17

17:                                               ; preds = %29, %16
  %18 = load i16, ptr %3, align 2, !tbaa !7
  %19 = load i16, ptr %2, align 2, !tbaa !7
  %20 = icmp slt i16 %18, %19
  br i1 %20, label %21, label %32

21:                                               ; preds = %17
  %22 = load i16, ptr %3, align 2, !tbaa !7
  %23 = add nsw i16 %22, 1
  %24 = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %23
  %25 = load i16, ptr %24, align 2, !tbaa !7
  %26 = add nsw i16 %25, 1
  %27 = load i16, ptr %3, align 2, !tbaa !7
  %28 = getelementptr inbounds [64 x i16], ptr @b, i16 0, i16 %27
  store i16 %26, ptr %28, align 2, !tbaa !7
  br label %29

29:                                               ; preds = %21
  %30 = load i16, ptr %3, align 2, !tbaa !7
  %31 = add nsw i16 %30, 1
  store i16 %31, ptr %3, align 2, !tbaa !7
  br label %17, !llvm.loop !15

32:                                               ; preds = %17
  %33 = load i16, ptr %2, align 2, !tbaa !7
  %34 = call i32 @total(i16 noundef signext %33)
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %34
}

; Function Attrs: nounwind
define internal i32 @over(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @clear()
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %17, %1
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = icmp slt i16 %5, %6
  br i1 %7, label %8, label %20

8:                                                ; preds = %4
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = add nsw i16 %9, 1
  %11 = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %10
  %12 = load i16, ptr %11, align 2, !tbaa !7
  %13 = load i16, ptr %3, align 2, !tbaa !7
  %14 = add nsw i16 %12, %13
  %15 = load i16, ptr %3, align 2, !tbaa !7
  %16 = getelementptr inbounds [64 x i16], ptr @b, i16 0, i16 %15
  store i16 %14, ptr %16, align 2, !tbaa !7
  br label %17

17:                                               ; preds = %8
  %18 = load i16, ptr %3, align 2, !tbaa !7
  %19 = add nsw i16 %18, 1
  store i16 %19, ptr %3, align 2, !tbaa !7
  br label %4, !llvm.loop !16

20:                                               ; preds = %4
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %21

21:                                               ; preds = %28, %20
  %22 = load i16, ptr %3, align 2, !tbaa !7
  %23 = load i16, ptr %2, align 2, !tbaa !7
  %24 = icmp slt i16 %22, %23
  br i1 %24, label %25, label %31

25:                                               ; preds = %21
  %26 = load i16, ptr %3, align 2, !tbaa !7
  %27 = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %26
  store i16 7, ptr %27, align 2, !tbaa !7
  br label %28

28:                                               ; preds = %25
  %29 = load i16, ptr %3, align 2, !tbaa !7
  %30 = add nsw i16 %29, 1
  store i16 %30, ptr %3, align 2, !tbaa !7
  br label %21, !llvm.loop !17

31:                                               ; preds = %21
  %32 = load i16, ptr %2, align 2, !tbaa !7
  %33 = call i32 @total(i16 noundef signext %32)
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %33
}

; Function Attrs: nounwind
define internal i32 @longer(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @clear()
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %13, %1
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = icmp slt i16 %5, %6
  br i1 %7, label %8, label %16

8:                                                ; preds = %4
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = mul nsw i16 %9, 3
  %11 = load i16, ptr %3, align 2, !tbaa !7
  %12 = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %11
  store i16 %10, ptr %12, align 2, !tbaa !7
  br label %13

13:                                               ; preds = %8
  %14 = load i16, ptr %3, align 2, !tbaa !7
  %15 = add nsw i16 %14, 1
  store i16 %15, ptr %3, align 2, !tbaa !7
  br label %4, !llvm.loop !18

16:                                               ; preds = %4
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %17

17:                                               ; preds = %29, %16
  %18 = load i16, ptr %3, align 2, !tbaa !7
  %19 = load i16, ptr %2, align 2, !tbaa !7
  %20 = add nsw i16 %19, 1
  %21 = icmp slt i16 %18, %20
  br i1 %21, label %22, label %32

22:                                               ; preds = %17
  %23 = load i16, ptr %3, align 2, !tbaa !7
  %24 = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %23
  %25 = load i16, ptr %24, align 2, !tbaa !7
  %26 = add nsw i16 %25, 1
  %27 = load i16, ptr %3, align 2, !tbaa !7
  %28 = getelementptr inbounds [64 x i16], ptr @b, i16 0, i16 %27
  store i16 %26, ptr %28, align 2, !tbaa !7
  br label %29

29:                                               ; preds = %22
  %30 = load i16, ptr %3, align 2, !tbaa !7
  %31 = add nsw i16 %30, 1
  store i16 %31, ptr %3, align 2, !tbaa !7
  br label %17, !llvm.loop !19

32:                                               ; preds = %17
  %33 = load i16, ptr %2, align 2, !tbaa !7
  %34 = call i32 @total(i16 noundef signext %33)
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %34
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #2

; Function Attrs: nounwind
define internal void @clear() #0 {
  %1 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store i16 0, ptr %1, align 2, !tbaa !7
  br label %2

2:                                                ; preds = %14, %0
  %3 = load i16, ptr %1, align 2, !tbaa !7
  %4 = icmp slt i16 %3, 64
  br i1 %4, label %5, label %17

5:                                                ; preds = %2
  %6 = load i16, ptr %1, align 2, !tbaa !7
  %7 = add nsw i16 100, %6
  %8 = load i16, ptr %1, align 2, !tbaa !7
  %9 = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %8
  store i16 %7, ptr %9, align 2, !tbaa !7
  %10 = load i16, ptr %1, align 2, !tbaa !7
  %11 = add nsw i16 200, %10
  %12 = load i16, ptr %1, align 2, !tbaa !7
  %13 = getelementptr inbounds [64 x i16], ptr @b, i16 0, i16 %12
  store i16 %11, ptr %13, align 2, !tbaa !7
  br label %14

14:                                               ; preds = %5
  %15 = load i16, ptr %1, align 2, !tbaa !7
  %16 = add nsw i16 %15, 1
  store i16 %16, ptr %1, align 2, !tbaa !7
  br label %2, !llvm.loop !20

17:                                               ; preds = %2
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret void
}

; Function Attrs: nounwind
define internal i32 @total(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i32, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i32 0, ptr %3, align 2, !tbaa !21
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %5

5:                                                ; preds = %21, %1
  %6 = load i16, ptr %4, align 2, !tbaa !7
  %7 = load i16, ptr %2, align 2, !tbaa !7
  %8 = icmp slt i16 %6, %7
  br i1 %8, label %9, label %24

9:                                                ; preds = %5
  %10 = load i16, ptr %4, align 2, !tbaa !7
  %11 = getelementptr inbounds [64 x i16], ptr @b, i16 0, i16 %10
  %12 = load i16, ptr %11, align 2, !tbaa !7
  %13 = load i16, ptr %4, align 2, !tbaa !7
  %14 = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %13
  %15 = load i16, ptr %14, align 2, !tbaa !7
  %16 = mul nsw i16 3, %15
  %17 = add nsw i16 %12, %16
  %18 = sext i16 %17 to i32
  %19 = load i32, ptr %3, align 2, !tbaa !21
  %20 = add nsw i32 %19, %18
  store i32 %20, ptr %3, align 2, !tbaa !21
  br label %21

21:                                               ; preds = %9
  %22 = load i16, ptr %4, align 2, !tbaa !7
  %23 = add nsw i16 %22, 1
  store i16 %23, ptr %4, align 2, !tbaa !7
  br label %5, !llvm.loop !23

24:                                               ; preds = %5
  %25 = load i32, ptr %3, align 2, !tbaa !21
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %25
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #2

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { nounwind }

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
!21 = !{!22, !22, i64 0}
!22 = !{!"long", !5, i64 0}
!23 = distinct !{!23, !10}
