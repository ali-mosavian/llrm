; ModuleID = 'nbody_shape.c'
source_filename = "nbody_shape.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

%struct.B = type { [3 x double], [3 x double], double }

; Function Attrs: noinline nounwind uwtable
define dso_local void @advance(ptr noundef %0, ptr noundef %1, double noundef %2) #0 {
  %4 = alloca ptr, align 8
  %5 = alloca ptr, align 8
  %6 = alloca double, align 8
  %7 = alloca [3 x double], align 8
  %8 = alloca double, align 8
  %9 = alloca i32, align 4
  %10 = alloca i32, align 4
  %11 = alloca i32, align 4
  store ptr %0, ptr %4, align 8
  store ptr %1, ptr %5, align 8
  store double %2, ptr %6, align 8
  store double 0.000000e+00, ptr %8, align 8
  store i32 0, ptr %9, align 4
  br label %12

12:                                               ; preds = %32, %3
  %13 = load i32, ptr %9, align 4
  %14 = icmp slt i32 %13, 3
  br i1 %14, label %15, label %35

15:                                               ; preds = %12
  %16 = load ptr, ptr %4, align 8
  %17 = getelementptr inbounds nuw %struct.B, ptr %16, i32 0, i32 0
  %18 = load i32, ptr %9, align 4
  %19 = sext i32 %18 to i64
  %20 = getelementptr inbounds [3 x double], ptr %17, i64 0, i64 %19
  %21 = load double, ptr %20, align 8
  %22 = load ptr, ptr %5, align 8
  %23 = getelementptr inbounds nuw %struct.B, ptr %22, i32 0, i32 0
  %24 = load i32, ptr %9, align 4
  %25 = sext i32 %24 to i64
  %26 = getelementptr inbounds [3 x double], ptr %23, i64 0, i64 %25
  %27 = load double, ptr %26, align 8
  %28 = fsub double %21, %27
  %29 = load i32, ptr %9, align 4
  %30 = sext i32 %29 to i64
  %31 = getelementptr inbounds [3 x double], ptr %7, i64 0, i64 %30
  store double %28, ptr %31, align 8
  br label %32

32:                                               ; preds = %15
  %33 = load i32, ptr %9, align 4
  %34 = add nsw i32 %33, 1
  store i32 %34, ptr %9, align 4
  br label %12, !llvm.loop !6

35:                                               ; preds = %12
  store i32 0, ptr %10, align 4
  br label %36

36:                                               ; preds = %50, %35
  %37 = load i32, ptr %10, align 4
  %38 = icmp slt i32 %37, 3
  br i1 %38, label %39, label %53

39:                                               ; preds = %36
  %40 = load i32, ptr %10, align 4
  %41 = sext i32 %40 to i64
  %42 = getelementptr inbounds [3 x double], ptr %7, i64 0, i64 %41
  %43 = load double, ptr %42, align 8
  %44 = load i32, ptr %10, align 4
  %45 = sext i32 %44 to i64
  %46 = getelementptr inbounds [3 x double], ptr %7, i64 0, i64 %45
  %47 = load double, ptr %46, align 8
  %48 = load double, ptr %8, align 8
  %49 = call double @llvm.fmuladd.f64(double %43, double %47, double %48)
  store double %49, ptr %8, align 8
  br label %50

50:                                               ; preds = %39
  %51 = load i32, ptr %10, align 4
  %52 = add nsw i32 %51, 1
  store i32 %52, ptr %10, align 4
  br label %36, !llvm.loop !8

53:                                               ; preds = %36
  store i32 0, ptr %11, align 4
  br label %54

54:                                               ; preds = %96, %53
  %55 = load i32, ptr %11, align 4
  %56 = icmp slt i32 %55, 3
  br i1 %56, label %57, label %99

57:                                               ; preds = %54
  %58 = load i32, ptr %11, align 4
  %59 = sext i32 %58 to i64
  %60 = getelementptr inbounds [3 x double], ptr %7, i64 0, i64 %59
  %61 = load double, ptr %60, align 8
  %62 = load ptr, ptr %5, align 8
  %63 = getelementptr inbounds nuw %struct.B, ptr %62, i32 0, i32 2
  %64 = load double, ptr %63, align 8
  %65 = fmul double %61, %64
  %66 = load double, ptr %6, align 8
  %67 = fmul double %65, %66
  %68 = load double, ptr %8, align 8
  %69 = fdiv double %67, %68
  %70 = load ptr, ptr %4, align 8
  %71 = getelementptr inbounds nuw %struct.B, ptr %70, i32 0, i32 1
  %72 = load i32, ptr %11, align 4
  %73 = sext i32 %72 to i64
  %74 = getelementptr inbounds [3 x double], ptr %71, i64 0, i64 %73
  %75 = load double, ptr %74, align 8
  %76 = fsub double %75, %69
  store double %76, ptr %74, align 8
  %77 = load i32, ptr %11, align 4
  %78 = sext i32 %77 to i64
  %79 = getelementptr inbounds [3 x double], ptr %7, i64 0, i64 %78
  %80 = load double, ptr %79, align 8
  %81 = load ptr, ptr %4, align 8
  %82 = getelementptr inbounds nuw %struct.B, ptr %81, i32 0, i32 2
  %83 = load double, ptr %82, align 8
  %84 = fmul double %80, %83
  %85 = load double, ptr %6, align 8
  %86 = fmul double %84, %85
  %87 = load double, ptr %8, align 8
  %88 = fdiv double %86, %87
  %89 = load ptr, ptr %5, align 8
  %90 = getelementptr inbounds nuw %struct.B, ptr %89, i32 0, i32 1
  %91 = load i32, ptr %11, align 4
  %92 = sext i32 %91 to i64
  %93 = getelementptr inbounds [3 x double], ptr %90, i64 0, i64 %92
  %94 = load double, ptr %93, align 8
  %95 = fadd double %94, %88
  store double %95, ptr %93, align 8
  br label %96

96:                                               ; preds = %57
  %97 = load i32, ptr %11, align 4
  %98 = add nsw i32 %97, 1
  store i32 %98, ptr %11, align 4
  br label %54, !llvm.loop !9

99:                                               ; preds = %54
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare double @llvm.fmuladd.f64(double, double, double) #1

attributes #0 = { noinline nounwind uwtable "frame-pointer"="non-leaf" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }

!llvm.module.flags = !{!0, !1, !2, !3, !4}
!llvm.ident = !{!5}

!0 = !{i32 1, !"wchar_size", i32 4}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"PIE Level", i32 2}
!3 = !{i32 7, !"uwtable", i32 2}
!4 = !{i32 7, !"frame-pointer", i32 1}
!5 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!6 = distinct !{!6, !7}
!7 = !{!"llvm.loop.mustprogress"}
!8 = distinct !{!8, !7}
!9 = distinct !{!9, !7}
