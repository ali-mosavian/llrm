target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
; ModuleID = 'rep_count.c'
source_filename = "rep_count.c"
target triple = "msp430"

@g = dso_local global [16 x [54 x i8]] zeroinitializer, align 1

; Function Attrs: nounwind
define dso_local i32 @bench() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i32 0, ptr %3, align 2, !tbaa !7
  store i16 0, ptr %1, align 2, !tbaa !9
  br label %4

4:                                                ; preds = %20, %0
  %5 = load i16, ptr %1, align 2, !tbaa !9
  %6 = icmp slt i16 %5, 16
  br i1 %6, label %7, label %23

7:                                                ; preds = %4
  store i16 0, ptr %2, align 2, !tbaa !9
  br label %8

8:                                                ; preds = %16, %7
  %9 = load i16, ptr %2, align 2, !tbaa !9
  %10 = icmp slt i16 %9, 54
  br i1 %10, label %11, label %19

11:                                               ; preds = %8
  %12 = load i16, ptr %1, align 2, !tbaa !9
  %13 = getelementptr inbounds [16 x [54 x i8]], ptr @g, i16 0, i16 %12
  %14 = load i16, ptr %2, align 2, !tbaa !9
  %15 = getelementptr inbounds [54 x i8], ptr %13, i16 0, i16 %14
  store i8 1, ptr %15, align 1, !tbaa !11
  br label %16

16:                                               ; preds = %11
  %17 = load i16, ptr %2, align 2, !tbaa !9
  %18 = add nsw i16 %17, 1
  store i16 %18, ptr %2, align 2, !tbaa !9
  br label %8, !llvm.loop !12

19:                                               ; preds = %8
  br label %20

20:                                               ; preds = %19
  %21 = load i16, ptr %1, align 2, !tbaa !9
  %22 = add nsw i16 %21, 1
  store i16 %22, ptr %1, align 2, !tbaa !9
  br label %4, !llvm.loop !15

23:                                               ; preds = %4
  store i16 0, ptr %1, align 2, !tbaa !9
  br label %24

24:                                               ; preds = %40, %23
  %25 = load i16, ptr %1, align 2, !tbaa !9
  %26 = icmp slt i16 %25, 16
  br i1 %26, label %27, label %43

27:                                               ; preds = %24
  store i16 0, ptr %2, align 2, !tbaa !9
  br label %28

28:                                               ; preds = %36, %27
  %29 = load i16, ptr %2, align 2, !tbaa !9
  %30 = icmp slt i16 %29, 16
  br i1 %30, label %31, label %39

31:                                               ; preds = %28
  %32 = load i16, ptr %1, align 2, !tbaa !9
  %33 = getelementptr inbounds [16 x [54 x i8]], ptr @g, i16 0, i16 %32
  %34 = load i16, ptr %2, align 2, !tbaa !9
  %35 = getelementptr inbounds [54 x i8], ptr %33, i16 0, i16 %34
  store i8 0, ptr %35, align 1, !tbaa !11
  br label %36

36:                                               ; preds = %31
  %37 = load i16, ptr %2, align 2, !tbaa !9
  %38 = add nsw i16 %37, 1
  store i16 %38, ptr %2, align 2, !tbaa !9
  br label %28, !llvm.loop !16

39:                                               ; preds = %28
  br label %40

40:                                               ; preds = %39
  %41 = load i16, ptr %1, align 2, !tbaa !9
  %42 = add nsw i16 %41, 1
  store i16 %42, ptr %1, align 2, !tbaa !9
  br label %24, !llvm.loop !17

43:                                               ; preds = %24
  store i16 0, ptr %1, align 2, !tbaa !9
  br label %44

44:                                               ; preds = %64, %43
  %45 = load i16, ptr %1, align 2, !tbaa !9
  %46 = icmp slt i16 %45, 16
  br i1 %46, label %47, label %67

47:                                               ; preds = %44
  store i16 0, ptr %2, align 2, !tbaa !9
  br label %48

48:                                               ; preds = %60, %47
  %49 = load i16, ptr %2, align 2, !tbaa !9
  %50 = icmp slt i16 %49, 54
  br i1 %50, label %51, label %63

51:                                               ; preds = %48
  %52 = load i16, ptr %1, align 2, !tbaa !9
  %53 = getelementptr inbounds [16 x [54 x i8]], ptr @g, i16 0, i16 %52
  %54 = load i16, ptr %2, align 2, !tbaa !9
  %55 = getelementptr inbounds [54 x i8], ptr %53, i16 0, i16 %54
  %56 = load i8, ptr %55, align 1, !tbaa !11
  %57 = sext i8 %56 to i32
  %58 = load i32, ptr %3, align 2, !tbaa !7
  %59 = add nsw i32 %58, %57
  store i32 %59, ptr %3, align 2, !tbaa !7
  br label %60

60:                                               ; preds = %51
  %61 = load i16, ptr %2, align 2, !tbaa !9
  %62 = add nsw i16 %61, 1
  store i16 %62, ptr %2, align 2, !tbaa !9
  br label %48, !llvm.loop !18

63:                                               ; preds = %48
  br label %64

64:                                               ; preds = %63
  %65 = load i16, ptr %1, align 2, !tbaa !9
  %66 = add nsw i16 %65, 1
  store i16 %66, ptr %1, align 2, !tbaa !9
  br label %44, !llvm.loop !19

67:                                               ; preds = %44
  %68 = load i32, ptr %3, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %68
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i32 @bench()
  call void @report(i32 noundef %2)
  ret i16 0
}

declare dso_local void @report(i32 noundef) #2

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
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
!7 = !{!8, !8, i64 0}
!8 = !{!"long", !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"short", !5, i64 0}
!11 = !{!5, !5, i64 0}
!12 = distinct !{!12, !13, !14}
!13 = !{!"llvm.loop.mustprogress"}
!14 = !{!"llvm.loop.unroll.disable"}
!15 = distinct !{!15, !13, !14}
!16 = distinct !{!16, !13, !14}
!17 = distinct !{!17, !13, !14}
!18 = distinct !{!18, !13, !14}
!19 = distinct !{!19, !13, !14}
