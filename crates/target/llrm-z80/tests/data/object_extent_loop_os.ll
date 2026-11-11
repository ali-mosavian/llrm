target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
; ModuleID = 'object_extent_loop.c'
source_filename = "object_extent_loop.c"
target triple = "msp430"

@A = internal global [12 x [12 x double]] zeroinitializer, align 2
@B = internal global [12 x [12 x double]] zeroinitializer, align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  call void @llvm.lifetime.start.p0(ptr %4) #4
  store i16 0, ptr %2, align 2, !tbaa !7
  br label %5

5:                                                ; preds = %35, %0
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = icmp slt i16 %6, 12
  br i1 %7, label %8, label %38

8:                                                ; preds = %5
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %9

9:                                                ; preds = %31, %8
  %10 = load i16, ptr %3, align 2, !tbaa !7
  %11 = icmp slt i16 %10, 12
  br i1 %11, label %12, label %34

12:                                               ; preds = %9
  %13 = load i16, ptr %2, align 2, !tbaa !7
  %14 = load i16, ptr %3, align 2, !tbaa !7
  %15 = add nsw i16 %13, %14
  %16 = sitofp i16 %15 to double
  %17 = fmul double %16, 5.000000e-01
  %18 = load i16, ptr %2, align 2, !tbaa !7
  %19 = getelementptr inbounds [12 x [12 x double]], ptr @A, i16 0, i16 %18
  %20 = load i16, ptr %3, align 2, !tbaa !7
  %21 = getelementptr inbounds [12 x double], ptr %19, i16 0, i16 %20
  store double %17, ptr %21, align 2, !tbaa !8
  %22 = load i16, ptr %2, align 2, !tbaa !7
  %23 = load i16, ptr %3, align 2, !tbaa !7
  %24 = sub nsw i16 %22, %23
  %25 = sitofp i16 %24 to double
  %26 = call double @llvm.fmuladd.f64(double %25, double 2.500000e-01, double 1.000000e+00)
  %27 = load i16, ptr %2, align 2, !tbaa !7
  %28 = getelementptr inbounds [12 x [12 x double]], ptr @B, i16 0, i16 %27
  %29 = load i16, ptr %3, align 2, !tbaa !7
  %30 = getelementptr inbounds [12 x double], ptr %28, i16 0, i16 %29
  store double %26, ptr %30, align 2, !tbaa !8
  br label %31

31:                                               ; preds = %12
  %32 = load i16, ptr %3, align 2, !tbaa !7
  %33 = add nsw i16 %32, 1
  store i16 %33, ptr %3, align 2, !tbaa !7
  br label %9, !llvm.loop !10

34:                                               ; preds = %9
  br label %35

35:                                               ; preds = %34
  %36 = load i16, ptr %2, align 2, !tbaa !7
  %37 = add nsw i16 %36, 1
  store i16 %37, ptr %2, align 2, !tbaa !7
  br label %5, !llvm.loop !12

38:                                               ; preds = %5
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %39

39:                                               ; preds = %52, %38
  %40 = load i16, ptr %4, align 2, !tbaa !7
  %41 = icmp sle i16 %40, 12
  br i1 %41, label %42, label %55

42:                                               ; preds = %39
  %43 = load i16, ptr %4, align 2, !tbaa !7
  %44 = call double @dot(i16 noundef %43, i16 noundef 3, i16 noundef 5) #5
  %45 = fmul double %44, 1.600000e+01
  %46 = fptosi double %45 to i32
  %47 = load i16, ptr %4, align 2, !tbaa !7
  %48 = call double @dot(i16 noundef %47, i16 noundef 11, i16 noundef 0) #5
  %49 = fmul double %48, 1.600000e+01
  %50 = fptosi double %49 to i32
  %51 = add nsw i32 %46, %50
  call void @report(i32 noundef %51) #5
  br label %52

52:                                               ; preds = %42
  %53 = load i16, ptr %4, align 2, !tbaa !7
  %54 = add nsw i16 %53, 1
  store i16 %54, ptr %4, align 2, !tbaa !7
  br label %39, !llvm.loop !13

55:                                               ; preds = %39
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare double @llvm.fmuladd.f64(double, double, double) #2

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #3

; Function Attrs: nounwind optsize
define internal double @dot(i16 noundef %0, i16 noundef %1, i16 noundef %2) #0 {
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  %6 = alloca i16, align 2
  %7 = alloca double, align 2
  %8 = alloca i16, align 2
  store i16 %0, ptr %4, align 2, !tbaa !7
  store i16 %1, ptr %5, align 2, !tbaa !7
  store i16 %2, ptr %6, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %7) #4
  store double 0.000000e+00, ptr %7, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %8) #4
  store i16 0, ptr %8, align 2, !tbaa !7
  br label %9

9:                                                ; preds = %26, %3
  %10 = load i16, ptr %8, align 2, !tbaa !7
  %11 = load i16, ptr %4, align 2, !tbaa !7
  %12 = icmp slt i16 %10, %11
  br i1 %12, label %13, label %29

13:                                               ; preds = %9
  %14 = load i16, ptr %5, align 2, !tbaa !7
  %15 = getelementptr inbounds [12 x [12 x double]], ptr @A, i16 0, i16 %14
  %16 = load i16, ptr %8, align 2, !tbaa !7
  %17 = getelementptr inbounds [12 x double], ptr %15, i16 0, i16 %16
  %18 = load double, ptr %17, align 2, !tbaa !8
  %19 = load i16, ptr %8, align 2, !tbaa !7
  %20 = getelementptr inbounds [12 x [12 x double]], ptr @B, i16 0, i16 %19
  %21 = load i16, ptr %6, align 2, !tbaa !7
  %22 = getelementptr inbounds [12 x double], ptr %20, i16 0, i16 %21
  %23 = load double, ptr %22, align 2, !tbaa !8
  %24 = load double, ptr %7, align 2, !tbaa !8
  %25 = call double @llvm.fmuladd.f64(double %18, double %23, double %24)
  store double %25, ptr %7, align 2, !tbaa !8
  br label %26

26:                                               ; preds = %13
  %27 = load i16, ptr %8, align 2, !tbaa !7
  %28 = add nsw i16 %27, 1
  store i16 %28, ptr %8, align 2, !tbaa !7
  br label %9, !llvm.loop !14

29:                                               ; preds = %9
  %30 = load double, ptr %7, align 2, !tbaa !8
  call void @llvm.lifetime.end.p0(ptr %8) #4
  call void @llvm.lifetime.end.p0(ptr %7) #4
  ret double %30
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
attributes #3 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #4 = { nounwind }
attributes #5 = { optsize }

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
!8 = !{!9, !9, i64 0}
!9 = !{!"double", !5, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
!13 = distinct !{!13, !11}
!14 = distinct !{!14, !11}
