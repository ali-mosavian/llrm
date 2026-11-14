; ModuleID = 'tests/run/c/huffman_decode.c'
source_filename = "tests/run/c/huffman_decode.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
target triple = "msp430"

@zero = dso_local global i32 0, align 2
@stream = internal global [16 x i8] zeroinitializer, align 1
@bitpos = internal global i16 0, align 2
@bad = internal global i16 0, align 2
@valptr = internal global [4 x [17 x i16]] zeroinitializer, align 2
@mincode = internal global [4 x [17 x i16]] zeroinitializer, align 2
@maxcode = internal global [4 x [18 x i16]] zeroinitializer, align 2
@huffval = internal global [4 x [256 x i8]] zeroinitializer, align 1

; Function Attrs: noinline nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca [17 x i16], align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %6

6:                                                ; preds = %12, %0
  %7 = load i16, ptr %4, align 2, !tbaa !7
  %8 = icmp slt i16 %7, 17
  br i1 %8, label %9, label %15

9:                                                ; preds = %6
  %10 = load i16, ptr %4, align 2, !tbaa !7
  %11 = getelementptr inbounds [17 x i16], ptr %2, i16 0, i16 %10
  store i16 0, ptr %11, align 2, !tbaa !7
  br label %12

12:                                               ; preds = %9
  %13 = load i16, ptr %4, align 2, !tbaa !7
  %14 = add nsw i16 %13, 1
  store i16 %14, ptr %4, align 2, !tbaa !7
  br label %6, !llvm.loop !8

15:                                               ; preds = %6
  %16 = getelementptr inbounds [17 x i16], ptr %2, i16 0, i16 2
  store i16 2, ptr %16, align 2, !tbaa !7
  %17 = getelementptr inbounds [17 x i16], ptr %2, i16 0, i16 3
  store i16 3, ptr %17, align 2, !tbaa !7
  %18 = getelementptr inbounds [17 x i16], ptr %2, i16 0, i16 5
  store i16 1, ptr %18, align 2, !tbaa !7
  store i16 6, ptr %5, align 2, !tbaa !7
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %26, %15
  %20 = load i16, ptr %3, align 2, !tbaa !7
  %21 = icmp slt i16 %20, 4
  br i1 %21, label %22, label %29

22:                                               ; preds = %19
  %23 = load i16, ptr %3, align 2, !tbaa !7
  %24 = getelementptr inbounds [17 x i16], ptr %2, i16 0, i16 0
  %25 = load i16, ptr %5, align 2, !tbaa !7
  call void @build(i16 noundef %23, ptr noundef %24, i16 noundef %25)
  br label %26

26:                                               ; preds = %22
  %27 = load i16, ptr %3, align 2, !tbaa !7
  %28 = add nsw i16 %27, 1
  store i16 %28, ptr %3, align 2, !tbaa !7
  br label %19, !llvm.loop !10

29:                                               ; preds = %19
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %30

30:                                               ; preds = %44, %29
  %31 = load i16, ptr %4, align 2, !tbaa !7
  %32 = icmp slt i16 %31, 16
  br i1 %32, label %33, label %47

33:                                               ; preds = %30
  %34 = load i16, ptr %4, align 2, !tbaa !7
  %35 = add nsw i16 %34, 1
  %36 = mul nsw i16 179, %35
  %37 = add nsw i16 %36, 85
  %38 = sext i16 %37 to i32
  %39 = load volatile i32, ptr @zero, align 2, !tbaa !11
  %40 = add nsw i32 %38, %39
  %41 = trunc i32 %40 to i8
  %42 = load i16, ptr %4, align 2, !tbaa !7
  %43 = getelementptr inbounds [16 x i8], ptr @stream, i16 0, i16 %42
  store i8 %41, ptr %43, align 1, !tbaa !13
  br label %44

44:                                               ; preds = %33
  %45 = load i16, ptr %4, align 2, !tbaa !7
  %46 = add nsw i16 %45, 1
  store i16 %46, ptr %4, align 2, !tbaa !7
  br label %30, !llvm.loop !14

47:                                               ; preds = %30
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %48

48:                                               ; preds = %72, %47
  %49 = load i16, ptr %3, align 2, !tbaa !7
  %50 = icmp slt i16 %49, 4
  br i1 %50, label %51, label %75

51:                                               ; preds = %48
  store i16 0, ptr @bitpos, align 2, !tbaa !7
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %52

52:                                               ; preds = %68, %51
  %53 = load i16, ptr %4, align 2, !tbaa !7
  %54 = icmp slt i16 %53, 12
  br i1 %54, label %55, label %59

55:                                               ; preds = %52
  %56 = load i16, ptr @bad, align 2, !tbaa !7
  %57 = icmp ne i16 %56, 0
  %58 = xor i1 %57, true
  br label %59

59:                                               ; preds = %55, %52
  %60 = phi i1 [ false, %52 ], [ %58, %55 ]
  br i1 %60, label %61, label %71

61:                                               ; preds = %59
  %62 = load i16, ptr %3, align 2, !tbaa !7
  %63 = call i16 @decode(i16 noundef %62)
  %64 = mul nsw i16 %63, 10
  %65 = load i16, ptr %3, align 2, !tbaa !7
  %66 = add nsw i16 %64, %65
  %67 = sext i16 %66 to i32
  call void @report(i32 noundef %67)
  br label %68

68:                                               ; preds = %61
  %69 = load i16, ptr %4, align 2, !tbaa !7
  %70 = add nsw i16 %69, 1
  store i16 %70, ptr %4, align 2, !tbaa !7
  br label %52, !llvm.loop !15

71:                                               ; preds = %59
  br label %72

72:                                               ; preds = %71
  %73 = load i16, ptr %3, align 2, !tbaa !7
  %74 = add nsw i16 %73, 1
  store i16 %74, ptr %3, align 2, !tbaa !7
  br label %48, !llvm.loop !16

75:                                               ; preds = %48
  %76 = load i16, ptr @bad, align 2, !tbaa !7
  %77 = sext i16 %76 to i32
  call void @report(i32 noundef %77)
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: noinline nounwind
define internal void @build(i16 noundef %0, ptr noundef %1, i16 noundef %2) #0 {
  %4 = alloca i16, align 2
  %5 = alloca ptr, align 2
  %6 = alloca i16, align 2
  %7 = alloca i16, align 2
  %8 = alloca i16, align 2
  %9 = alloca i16, align 2
  store i16 %0, ptr %4, align 2, !tbaa !7
  store ptr %1, ptr %5, align 2, !tbaa !17
  store i16 %2, ptr %6, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %7) #3
  call void @llvm.lifetime.start.p0(ptr %8) #3
  store i16 0, ptr %8, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i16 0, ptr %9, align 2, !tbaa !7
  store i16 1, ptr %7, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %53, %3
  %11 = load i16, ptr %7, align 2, !tbaa !7
  %12 = icmp sle i16 %11, 16
  br i1 %12, label %13, label %56

13:                                               ; preds = %10
  %14 = load i16, ptr %9, align 2, !tbaa !7
  %15 = load i16, ptr %4, align 2, !tbaa !7
  %16 = getelementptr inbounds [4 x [17 x i16]], ptr @valptr, i16 0, i16 %15
  %17 = load i16, ptr %7, align 2, !tbaa !7
  %18 = getelementptr inbounds [17 x i16], ptr %16, i16 0, i16 %17
  store i16 %14, ptr %18, align 2, !tbaa !7
  %19 = load i16, ptr %8, align 2, !tbaa !7
  %20 = load i16, ptr %4, align 2, !tbaa !7
  %21 = getelementptr inbounds [4 x [17 x i16]], ptr @mincode, i16 0, i16 %20
  %22 = load i16, ptr %7, align 2, !tbaa !7
  %23 = getelementptr inbounds [17 x i16], ptr %21, i16 0, i16 %22
  store i16 %19, ptr %23, align 2, !tbaa !7
  %24 = load ptr, ptr %5, align 2, !tbaa !17
  %25 = load i16, ptr %7, align 2, !tbaa !7
  %26 = getelementptr inbounds i16, ptr %24, i16 %25
  %27 = load i16, ptr %26, align 2, !tbaa !7
  %28 = load i16, ptr %8, align 2, !tbaa !7
  %29 = add nsw i16 %28, %27
  store i16 %29, ptr %8, align 2, !tbaa !7
  %30 = load ptr, ptr %5, align 2, !tbaa !17
  %31 = load i16, ptr %7, align 2, !tbaa !7
  %32 = getelementptr inbounds i16, ptr %30, i16 %31
  %33 = load i16, ptr %32, align 2, !tbaa !7
  %34 = load i16, ptr %9, align 2, !tbaa !7
  %35 = add nsw i16 %34, %33
  store i16 %35, ptr %9, align 2, !tbaa !7
  %36 = load ptr, ptr %5, align 2, !tbaa !17
  %37 = load i16, ptr %7, align 2, !tbaa !7
  %38 = getelementptr inbounds i16, ptr %36, i16 %37
  %39 = load i16, ptr %38, align 2, !tbaa !7
  %40 = icmp ne i16 %39, 0
  br i1 %40, label %41, label %44

41:                                               ; preds = %13
  %42 = load i16, ptr %8, align 2, !tbaa !7
  %43 = sub nsw i16 %42, 1
  br label %45

44:                                               ; preds = %13
  br label %45

45:                                               ; preds = %44, %41
  %46 = phi i16 [ %43, %41 ], [ -1, %44 ]
  %47 = load i16, ptr %4, align 2, !tbaa !7
  %48 = getelementptr inbounds [4 x [18 x i16]], ptr @maxcode, i16 0, i16 %47
  %49 = load i16, ptr %7, align 2, !tbaa !7
  %50 = getelementptr inbounds [18 x i16], ptr %48, i16 0, i16 %49
  store i16 %46, ptr %50, align 2, !tbaa !7
  %51 = load i16, ptr %8, align 2, !tbaa !7
  %52 = mul nsw i16 %51, 2
  store i16 %52, ptr %8, align 2, !tbaa !7
  br label %53

53:                                               ; preds = %45
  %54 = load i16, ptr %7, align 2, !tbaa !7
  %55 = add nsw i16 %54, 1
  store i16 %55, ptr %7, align 2, !tbaa !7
  br label %10, !llvm.loop !20

56:                                               ; preds = %10
  store i16 0, ptr %7, align 2, !tbaa !7
  br label %57

57:                                               ; preds = %72, %56
  %58 = load i16, ptr %7, align 2, !tbaa !7
  %59 = load i16, ptr %6, align 2, !tbaa !7
  %60 = icmp slt i16 %58, %59
  br i1 %60, label %61, label %75

61:                                               ; preds = %57
  %62 = load i16, ptr %7, align 2, !tbaa !7
  %63 = mul nsw i16 3, %62
  %64 = add nsw i16 10, %63
  %65 = load i16, ptr %4, align 2, !tbaa !7
  %66 = add nsw i16 %64, %65
  %67 = trunc i16 %66 to i8
  %68 = load i16, ptr %4, align 2, !tbaa !7
  %69 = getelementptr inbounds [4 x [256 x i8]], ptr @huffval, i16 0, i16 %68
  %70 = load i16, ptr %7, align 2, !tbaa !7
  %71 = getelementptr inbounds [256 x i8], ptr %69, i16 0, i16 %70
  store i8 %67, ptr %71, align 1, !tbaa !13
  br label %72

72:                                               ; preds = %61
  %73 = load i16, ptr %7, align 2, !tbaa !7
  %74 = add nsw i16 %73, 1
  store i16 %74, ptr %7, align 2, !tbaa !7
  br label %57, !llvm.loop !21

75:                                               ; preds = %57
  call void @llvm.lifetime.end.p0(ptr %9) #3
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  ret void
}

declare dso_local void @report(i32 noundef) #2

; Function Attrs: noinline nounwind
define internal i16 @decode(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  %6 = alloca i32, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %4) #3
  %7 = call i16 @getbit()
  store i16 %7, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 1, ptr %5, align 2, !tbaa !7
  br label %8

8:                                                ; preds = %25, %1
  %9 = load i16, ptr %4, align 2, !tbaa !7
  %10 = load i16, ptr %3, align 2, !tbaa !7
  %11 = getelementptr inbounds [4 x [18 x i16]], ptr @maxcode, i16 0, i16 %10
  %12 = load i16, ptr %5, align 2, !tbaa !7
  %13 = getelementptr inbounds [18 x i16], ptr %11, i16 0, i16 %12
  %14 = load i16, ptr %13, align 2, !tbaa !7
  %15 = icmp sgt i16 %9, %14
  br i1 %15, label %16, label %26

16:                                               ; preds = %8
  %17 = load i16, ptr %4, align 2, !tbaa !7
  %18 = mul nsw i16 %17, 2
  %19 = call i16 @getbit()
  %20 = add nsw i16 %18, %19
  store i16 %20, ptr %4, align 2, !tbaa !7
  %21 = load i16, ptr %5, align 2, !tbaa !7
  %22 = add nsw i16 %21, 1
  store i16 %22, ptr %5, align 2, !tbaa !7
  %23 = icmp sgt i16 %22, 16
  br i1 %23, label %24, label %25

24:                                               ; preds = %16
  store i16 1, ptr @bad, align 2, !tbaa !7
  store i16 0, ptr %2, align 2
  store i32 1, ptr %6, align 2
  br label %45

25:                                               ; preds = %16
  br label %8, !llvm.loop !22

26:                                               ; preds = %8
  %27 = load i16, ptr %3, align 2, !tbaa !7
  %28 = getelementptr inbounds [4 x [256 x i8]], ptr @huffval, i16 0, i16 %27
  %29 = load i16, ptr %3, align 2, !tbaa !7
  %30 = getelementptr inbounds [4 x [17 x i16]], ptr @valptr, i16 0, i16 %29
  %31 = load i16, ptr %5, align 2, !tbaa !7
  %32 = getelementptr inbounds [17 x i16], ptr %30, i16 0, i16 %31
  %33 = load i16, ptr %32, align 2, !tbaa !7
  %34 = load i16, ptr %4, align 2, !tbaa !7
  %35 = add nsw i16 %33, %34
  %36 = load i16, ptr %3, align 2, !tbaa !7
  %37 = getelementptr inbounds [4 x [17 x i16]], ptr @mincode, i16 0, i16 %36
  %38 = load i16, ptr %5, align 2, !tbaa !7
  %39 = getelementptr inbounds [17 x i16], ptr %37, i16 0, i16 %38
  %40 = load i16, ptr %39, align 2, !tbaa !7
  %41 = sub nsw i16 %35, %40
  %42 = getelementptr inbounds [256 x i8], ptr %28, i16 0, i16 %41
  %43 = load i8, ptr %42, align 1, !tbaa !13
  %44 = zext i8 %43 to i16
  store i16 %44, ptr %2, align 2
  store i32 1, ptr %6, align 2
  br label %45

45:                                               ; preds = %26, %24
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  %46 = load i16, ptr %2, align 2
  ret i16 %46
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: noinline nounwind
define internal i16 @getbit() #0 {
  %1 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  %2 = load i16, ptr @bitpos, align 2, !tbaa !7
  %3 = ashr i16 %2, 3
  %4 = getelementptr inbounds [16 x i8], ptr @stream, i16 0, i16 %3
  %5 = load i8, ptr %4, align 1, !tbaa !13
  %6 = zext i8 %5 to i16
  %7 = load i16, ptr @bitpos, align 2, !tbaa !7
  %8 = and i16 %7, 7
  %9 = sub nsw i16 7, %8
  %10 = ashr i16 %6, %9
  %11 = and i16 %10, 1
  store i16 %11, ptr %1, align 2, !tbaa !7
  %12 = load i16, ptr @bitpos, align 2, !tbaa !7
  %13 = add nsw i16 %12, 1
  store i16 %13, ptr @bitpos, align 2, !tbaa !7
  %14 = load i16, ptr %1, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i16 %14
}

attributes #0 = { noinline nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
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
!7 = !{!4, !4, i64 0}
!8 = distinct !{!8, !9}
!9 = !{!"llvm.loop.mustprogress"}
!10 = distinct !{!10, !9}
!11 = !{!12, !12, i64 0}
!12 = !{!"long", !5, i64 0}
!13 = !{!5, !5, i64 0}
!14 = distinct !{!14, !9}
!15 = distinct !{!15, !9}
!16 = distinct !{!16, !9}
!17 = !{!18, !18, i64 0}
!18 = !{!"p1 int", !19, i64 0}
!19 = !{!"any pointer", !5, i64 0}
!20 = distinct !{!20, !9}
!21 = distinct !{!21, !9}
!22 = distinct !{!22, !9}
