; ModuleID = '/home/alim/scratch/handoff/greedy-nonempty-interval-seed100.c'
source_filename = "/home/alim/scratch/handoff/greedy-nonempty-interval-seed100.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

%struct.S = type { i8, i16, i8 }

@buf = external dso_local constant [1 x i8], align 1

; Function Attrs: nounwind
define dso_local i16 @test() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i16, align 2
  %5 = alloca i8, align 1
  %6 = alloca i8, align 1
  %7 = alloca i8, align 1
  %8 = alloca i8, align 1
  %9 = alloca i8, align 1
  %10 = alloca i16, align 2
  %11 = alloca %struct.S, align 2
  call void @llvm.lifetime.start.p0(i64 4, ptr %1) #2
  store i32 1, ptr %1, align 2, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 4, ptr %2) #2
  store i32 1, ptr %2, align 2, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 4, ptr %3) #2
  store i32 2147483647, ptr %3, align 2, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 2, ptr %4) #2
  store i16 1, ptr %4, align 2, !tbaa !6
  call void @llvm.lifetime.start.p0(i64 1, ptr %5) #2
  call void @llvm.lifetime.start.p0(i64 1, ptr %6) #2
  call void @llvm.lifetime.start.p0(i64 1, ptr %7) #2
  call void @llvm.lifetime.start.p0(i64 1, ptr %8) #2
  call void @llvm.lifetime.start.p0(i64 1, ptr %9) #2
  call void @llvm.lifetime.start.p0(i64 2, ptr %10) #2
  store i16 1, ptr %10, align 2, !tbaa !6
  call void @llvm.lifetime.start.p0(i64 6, ptr %11) #2
  %12 = getelementptr inbounds nuw %struct.S, ptr %11, i32 0, i32 0
  store i8 1, ptr %12, align 2, !tbaa !8
  %13 = getelementptr inbounds nuw %struct.S, ptr %11, i32 0, i32 1
  store i16 1, ptr %13, align 2, !tbaa !10
  %14 = getelementptr inbounds nuw %struct.S, ptr %11, i32 0, i32 2
  store i8 0, ptr %14, align 2, !tbaa !11
  store i8 0, ptr %6, align 1, !tbaa !12
  br label %15

15:                                               ; preds = %50, %0
  %16 = load i8, ptr %6, align 1, !tbaa !12
  %17 = zext i8 %16 to i16
  %18 = icmp slt i16 %17, 9
  br i1 %18, label %19, label %53

19:                                               ; preds = %15
  %20 = zext i8 ptrtoint (ptr @h0 to i8) to i32
  %21 = icmp ugt i32 %20, 1
  br i1 %21, label %22, label %23

22:                                               ; preds = %19
  br label %23

23:                                               ; preds = %22, %19
  %24 = phi i1 [ false, %19 ], [ true, %22 ]
  %25 = zext i1 %24 to i16
  %26 = sext i16 %25 to i32
  store i32 %26, ptr %3, align 2, !tbaa !2
  %27 = load i8, ptr getelementptr inbounds ([1 x i8], ptr @buf, i16 0, i16 6), align 1, !tbaa !12
  %28 = zext i8 %27 to i16
  %29 = trunc i16 %28 to i8
  %30 = zext i8 %29 to i16
  %31 = load i32, ptr %1, align 2, !tbaa !2
  %32 = trunc i32 %31 to i8
  %33 = zext i8 %32 to i16
  %34 = icmp slt i16 %30, %33
  br i1 %34, label %35, label %39

35:                                               ; preds = %23
  %36 = load i32, ptr %1, align 2, !tbaa !2
  %37 = trunc i32 %36 to i8
  %38 = zext i8 %37 to i16
  br label %47

39:                                               ; preds = %23
  %40 = load i16, ptr %4, align 2, !tbaa !6
  %41 = trunc i16 %40 to i8
  %42 = zext i8 %41 to i16
  %43 = load i32, ptr %2, align 2, !tbaa !2
  %44 = trunc i32 %43 to i8
  %45 = zext i8 %44 to i16
  %46 = xor i16 %42, %45
  br label %47

47:                                               ; preds = %39, %35
  %48 = phi i16 [ %38, %35 ], [ %46, %39 ]
  %49 = trunc i16 %48 to i8
  call void @bump(ptr noundef %11, i8 noundef zeroext %49)
  br label %50

50:                                               ; preds = %47
  %51 = load i8, ptr %6, align 1, !tbaa !12
  %52 = add i8 %51, 1
  store i8 %52, ptr %6, align 1, !tbaa !12
  br label %15, !llvm.loop !13

53:                                               ; preds = %15
  br label %54

54:                                               ; preds = %85, %53
  %55 = load i8, ptr %6, align 1, !tbaa !12
  %56 = icmp ne i8 %55, 0
  br i1 %56, label %57, label %86

57:                                               ; preds = %54
  %58 = load i8, ptr %6, align 1, !tbaa !12
  %59 = add i8 %58, -1
  store i8 %59, ptr %6, align 1, !tbaa !12
  store i8 0, ptr %7, align 1, !tbaa !12
  br label %60

60:                                               ; preds = %82, %57
  %61 = load i8, ptr %7, align 1, !tbaa !12
  %62 = zext i8 %61 to i16
  %63 = icmp slt i16 %62, 7
  br i1 %63, label %64, label %85

64:                                               ; preds = %60
  %65 = load i32, ptr %3, align 2, !tbaa !2
  %66 = load i32, ptr %1, align 2, !tbaa !2
  %67 = trunc i32 %66 to i8
  %68 = sext i8 %67 to i32
  %69 = load i32, ptr %3, align 2, !tbaa !2
  %70 = trunc i32 %69 to i8
  %71 = sext i8 %70 to i32
  %72 = icmp ult i32 %68, %71
  %73 = zext i1 %72 to i16
  %74 = sext i16 %73 to i32
  %75 = load i32, ptr %2, align 2, !tbaa !2
  %76 = and i32 %74, %75
  %77 = load i32, ptr %1, align 2, !tbaa !2
  %78 = and i32 %76, %77
  %79 = xor i32 %65, %78
  store i32 %79, ptr %3, align 2, !tbaa !2
  %80 = load i32, ptr %1, align 2, !tbaa !2
  %81 = add i32 %80, 1
  store i32 %81, ptr %1, align 2, !tbaa !2
  br label %82

82:                                               ; preds = %64
  %83 = load i8, ptr %7, align 1, !tbaa !12
  %84 = add i8 %83, 1
  store i8 %84, ptr %7, align 1, !tbaa !12
  br label %60, !llvm.loop !15

85:                                               ; preds = %60
  br label %54, !llvm.loop !16

86:                                               ; preds = %54
  %87 = zext i8 ptrtoint (ptr @h0 to i8) to i16
  %88 = and i16 %87, 1
  switch i16 %88, label %89 [
  ]

89:                                               ; preds = %86
  %90 = getelementptr inbounds nuw %struct.S, ptr %11, i32 0, i32 0
  %91 = load i8, ptr %90, align 2, !tbaa !8
  %92 = zext i8 %91 to i16
  %93 = xor i16 0, %92
  %94 = getelementptr inbounds nuw %struct.S, ptr %11, i32 0, i32 1
  %95 = load i16, ptr %94, align 2, !tbaa !10
  %96 = xor i16 %93, %95
  %97 = getelementptr inbounds nuw %struct.S, ptr %11, i32 0, i32 2
  %98 = load i8, ptr %97, align 2, !tbaa !11
  %99 = zext i8 %98 to i16
  %100 = xor i16 %96, %99
  store i16 %100, ptr %10, align 2, !tbaa !6
  %101 = load i32, ptr %3, align 2, !tbaa !2
  %102 = trunc i32 %101 to i16
  %103 = load i8, ptr getelementptr inbounds ([1 x i8], ptr @buf, i16 0, i16 6), align 1, !tbaa !12
  %104 = zext i8 %103 to i16
  %105 = urem i16 %102, %104
  %106 = load i16, ptr %10, align 2, !tbaa !6
  %107 = xor i16 0, %106
  %108 = or i16 %105, %107
  call void @llvm.lifetime.end.p0(i64 6, ptr %11) #2
  call void @llvm.lifetime.end.p0(i64 2, ptr %10) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %9) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %8) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %7) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %6) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %5) #2
  call void @llvm.lifetime.end.p0(i64 2, ptr %4) #2
  call void @llvm.lifetime.end.p0(i64 4, ptr %3) #2
  call void @llvm.lifetime.end.p0(i64 4, ptr %2) #2
  call void @llvm.lifetime.end.p0(i64 4, ptr %1) #2
  ret i16 %108
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) #1

; Function Attrs: nounwind
define internal zeroext i8 @h0() #0 {
  %1 = alloca i8, align 1
  %2 = alloca [1 x i32], align 2
  %3 = alloca [1 x i8], align 1
  %4 = alloca i8, align 1
  %5 = alloca i8, align 1
  %6 = alloca i8, align 1
  %7 = alloca i8, align 1
  %8 = alloca i8, align 1
  call void @llvm.lifetime.start.p0(i64 4, ptr %2) #2
  call void @llvm.lifetime.start.p0(i64 1, ptr %3) #2
  call void @llvm.lifetime.start.p0(i64 1, ptr %4) #2
  call void @llvm.lifetime.start.p0(i64 1, ptr %5) #2
  call void @llvm.lifetime.start.p0(i64 1, ptr %6) #2
  call void @llvm.lifetime.start.p0(i64 1, ptr %7) #2
  call void @llvm.lifetime.start.p0(i64 1, ptr %8) #2
  store i8 0, ptr %4, align 1, !tbaa !12
  br label %9

9:                                                ; preds = %17, %0
  %10 = load i8, ptr %4, align 1, !tbaa !12
  %11 = zext i8 %10 to i16
  %12 = icmp slt i16 %11, 1
  br i1 %12, label %13, label %20

13:                                               ; preds = %9
  %14 = load i8, ptr %4, align 1, !tbaa !12
  %15 = zext i8 %14 to i16
  %16 = getelementptr inbounds nuw [1 x i32], ptr %2, i16 0, i16 %15
  store i32 1, ptr %16, align 2, !tbaa !2
  br label %17

17:                                               ; preds = %13
  %18 = load i8, ptr %4, align 1, !tbaa !12
  %19 = add i8 %18, 1
  store i8 %19, ptr %4, align 1, !tbaa !12
  br label %9, !llvm.loop !17

20:                                               ; preds = %9
  store i8 0, ptr %4, align 1, !tbaa !12
  br label %21

21:                                               ; preds = %29, %20
  %22 = load i8, ptr %4, align 1, !tbaa !12
  %23 = zext i8 %22 to i16
  %24 = icmp slt i16 %23, 1
  br i1 %24, label %25, label %32

25:                                               ; preds = %21
  %26 = load i8, ptr %4, align 1, !tbaa !12
  %27 = zext i8 %26 to i16
  %28 = getelementptr inbounds nuw [1 x i8], ptr %3, i16 0, i16 %27
  store i8 1, ptr %28, align 1, !tbaa !12
  br label %29

29:                                               ; preds = %25
  %30 = load i8, ptr %4, align 1, !tbaa !12
  %31 = add i8 %30, 1
  store i8 %31, ptr %4, align 1, !tbaa !12
  br label %21, !llvm.loop !18

32:                                               ; preds = %21
  call void @llvm.lifetime.end.p0(i64 1, ptr %8) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %7) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %6) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %5) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %4) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %3) #2
  call void @llvm.lifetime.end.p0(i64 4, ptr %2) #2
  %33 = load i8, ptr %1, align 1
  ret i8 %33
}

; Function Attrs: nounwind
define internal void @bump(ptr noundef %0, i8 noundef zeroext %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca i8, align 1
  store ptr %0, ptr %3, align 2, !tbaa !19
  store i8 %1, ptr %4, align 1, !tbaa !12
  %5 = load ptr, ptr %3, align 2, !tbaa !19
  %6 = getelementptr inbounds nuw %struct.S, ptr %5, i32 0, i32 0
  %7 = load i8, ptr %6, align 2, !tbaa !8
  %8 = zext i8 %7 to i16
  %9 = load i8, ptr %4, align 1, !tbaa !12
  %10 = zext i8 %9 to i16
  %11 = add nsw i16 %8, %10
  %12 = trunc i16 %11 to i8
  %13 = load ptr, ptr %3, align 2, !tbaa !19
  %14 = getelementptr inbounds nuw %struct.S, ptr %13, i32 0, i32 0
  store i8 %12, ptr %14, align 2, !tbaa !8
  %15 = load ptr, ptr %3, align 2, !tbaa !19
  %16 = getelementptr inbounds nuw %struct.S, ptr %15, i32 0, i32 1
  %17 = load i16, ptr %16, align 2, !tbaa !10
  %18 = load i8, ptr %4, align 1, !tbaa !12
  %19 = zext i8 %18 to i16
  %20 = shl i16 %19, 1
  %21 = xor i16 %17, %20
  %22 = load ptr, ptr %3, align 2, !tbaa !19
  %23 = getelementptr inbounds nuw %struct.S, ptr %22, i32 0, i32 1
  store i16 %21, ptr %23, align 2, !tbaa !10
  %24 = load i8, ptr %4, align 1, !tbaa !12
  %25 = zext i8 %24 to i16
  %26 = and i16 %25, 1
  %27 = icmp ne i16 %26, 0
  br i1 %27, label %28, label %31

28:                                               ; preds = %2
  %29 = load ptr, ptr %3, align 2, !tbaa !19
  %30 = getelementptr inbounds nuw %struct.S, ptr %29, i32 0, i32 2
  store i8 1, ptr %30, align 2, !tbaa !11
  br label %31

31:                                               ; preds = %28, %2
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) #1

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!2 = !{!3, !3, i64 0}
!3 = !{!"long", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!7, !7, i64 0}
!7 = !{!"int", !4, i64 0}
!8 = !{!9, !4, i64 0}
!9 = !{!"S", !4, i64 0, !7, i64 2, !4, i64 4}
!10 = !{!9, !7, i64 2}
!11 = !{!9, !4, i64 4}
!12 = !{!4, !4, i64 0}
!13 = distinct !{!13, !14}
!14 = !{!"llvm.loop.mustprogress"}
!15 = distinct !{!15, !14}
!16 = distinct !{!16, !14}
!17 = distinct !{!17, !14}
!18 = distinct !{!18, !14}
!19 = !{!20, !20, i64 0}
!20 = !{!"p1 _ZTS1S", !21, i64 0}
!21 = !{!"any pointer", !4, i64 0}
