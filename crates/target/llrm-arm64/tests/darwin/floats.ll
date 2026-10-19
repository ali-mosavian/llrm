source_filename = "floats.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@.str = private unnamed_addr constant [7 x i8] c"%d %d\0A\00", align 1
@.str.1 = private unnamed_addr constant [4 x i8] c"%d\0A\00", align 1
@.str.2 = private unnamed_addr constant [10 x i8] c"%d %d %d\0A\00", align 1
@.str.3 = private unnamed_addr constant [8 x i8] c"%ld %d\0A\00", align 1
@.str.4 = private unnamed_addr constant [5 x i8] c"%lu\0A\00", align 1

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca double, align 8
  %3 = alloca i32, align 4
  %4 = alloca double, align 8
  %5 = alloca [8 x float], align 4
  %6 = alloca i32, align 4
  %7 = alloca i64, align 8
  %8 = alloca double, align 8
  %9 = alloca i64, align 8
  store i32 0, ptr %1, align 4
  store double 1.250000e+00, ptr %2, align 8, !tbaa !9
  store i32 0, ptr %3, align 4, !tbaa !11
  br label %10

10:                                               ; preds = %25, %0
  %11 = load i32, ptr %3, align 4, !tbaa !11
  %12 = icmp slt i32 %11, 6
  br i1 %12, label %13, label %28

13:                                               ; preds = %10
  %14 = load double, ptr %2, align 8, !tbaa !9
  %15 = load i32, ptr %3, align 4, !tbaa !11
  %16 = sitofp i32 %15 to double
  %17 = fadd double %14, %16
  %18 = call double @poly(double noundef %17)
  store double %18, ptr %4, align 8, !tbaa !9
  %19 = load double, ptr %4, align 8, !tbaa !9
  %20 = fmul double %19, 1.000000e+02
  %21 = fptosi double %20 to i32
  %22 = load double, ptr %4, align 8, !tbaa !9
  %23 = fptosi double %22 to i32
  %24 = call i32 (ptr, ...) @printf(ptr noundef @.str, i32 noundef %21, i32 noundef %23)
  br label %25

25:                                               ; preds = %13
  %26 = load i32, ptr %3, align 4, !tbaa !11
  %27 = add nsw i32 %26, 1
  store i32 %27, ptr %3, align 4, !tbaa !11
  br label %10, !llvm.loop !12

28:                                               ; preds = %10
  store i32 0, ptr %6, align 4, !tbaa !11
  br label %29

29:                                               ; preds = %39, %28
  %30 = load i32, ptr %6, align 4, !tbaa !11
  %31 = icmp slt i32 %30, 8
  br i1 %31, label %32, label %42

32:                                               ; preds = %29
  %33 = load i32, ptr %6, align 4, !tbaa !11
  %34 = sitofp i32 %33 to float
  %35 = fmul float %34, 1.500000e+00
  %36 = load i32, ptr %6, align 4, !tbaa !11
  %37 = sext i32 %36 to i64
  %38 = getelementptr inbounds [8 x float], ptr %5, i64 0, i64 %37
  store float %35, ptr %38, align 4, !tbaa !14
  br label %39

39:                                               ; preds = %32
  %40 = load i32, ptr %6, align 4, !tbaa !11
  %41 = add nsw i32 %40, 1
  store i32 %41, ptr %6, align 4, !tbaa !11
  br label %29, !llvm.loop !16

42:                                               ; preds = %29
  %43 = getelementptr inbounds [8 x float], ptr %5, i64 0, i64 0
  %44 = call float @fsum(ptr noundef %43, i32 noundef 8)
  %45 = fmul float %44, 1.000000e+01
  %46 = fptosi float %45 to i32
  %47 = call i32 (ptr, ...) @printf(ptr noundef @.str.1, i32 noundef %46)
  %48 = call i32 @cmp(double noundef 1.000000e+00, double noundef 2.000000e+00)
  %49 = call i32 @cmp(double noundef 2.000000e+00, double noundef 1.000000e+00)
  %50 = call i32 @cmp(double noundef 1.000000e+00, double noundef 1.000000e+00)
  %51 = call i32 (ptr, ...) @printf(ptr noundef @.str.2, i32 noundef %48, i32 noundef %49, i32 noundef %50)
  store i64 123456789012345, ptr %7, align 8, !tbaa !17
  %52 = load i64, ptr %7, align 8, !tbaa !17
  %53 = sitofp i64 %52 to double
  store double %53, ptr %8, align 8, !tbaa !9
  %54 = load double, ptr %8, align 8, !tbaa !9
  %55 = fdiv double %54, 3.000000e+00
  %56 = fptosi double %55 to i64
  %57 = load double, ptr %8, align 8, !tbaa !9
  %58 = fdiv double %57, 1.000000e+09
  %59 = fptrunc double %58 to float
  %60 = fptosi float %59 to i32
  %61 = srem i32 %60, 1000
  %62 = call i32 (ptr, ...) @printf(ptr noundef @.str.3, i64 noundef %56, i32 noundef %61)
  store i64 -446744073709551616, ptr %9, align 8, !tbaa !17
  %63 = load i64, ptr %9, align 8, !tbaa !17
  %64 = uitofp i64 %63 to double
  %65 = fptoui double %64 to i64
  %66 = udiv i64 %65, 1000
  %67 = call i32 (ptr, ...) @printf(ptr noundef @.str.4, i64 noundef %66)
  ret i32 0
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal double @poly(double noundef %0) #0 {
  %2 = alloca double, align 8
  store double %0, ptr %2, align 8, !tbaa !9
  %3 = load double, ptr %2, align 8, !tbaa !9
  %4 = call double @llvm.fmuladd.f64(double 3.000000e+00, double %3, double 2.000000e+00)
  %5 = load double, ptr %2, align 8, !tbaa !9
  %6 = call double @llvm.fmuladd.f64(double %4, double %5, double -5.000000e+00)
  %7 = load double, ptr %2, align 8, !tbaa !9
  %8 = call double @llvm.fmuladd.f64(double %6, double %7, double 5.000000e-01)
  ret double %8
}

declare i32 @printf(ptr noundef, ...) #1

; Function Attrs: nounwind ssp uwtable(sync)
define internal float @fsum(ptr noundef %0, i32 noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca i32, align 4
  %5 = alloca float, align 4
  %6 = alloca i32, align 4
  store ptr %0, ptr %3, align 8, !tbaa !19
  store i32 %1, ptr %4, align 4, !tbaa !11
  store float 0.000000e+00, ptr %5, align 4, !tbaa !14
  store i32 0, ptr %6, align 4, !tbaa !11
  br label %7

7:                                                ; preds = %19, %2
  %8 = load i32, ptr %6, align 4, !tbaa !11
  %9 = load i32, ptr %4, align 4, !tbaa !11
  %10 = icmp slt i32 %8, %9
  br i1 %10, label %11, label %22

11:                                               ; preds = %7
  %12 = load ptr, ptr %3, align 8, !tbaa !19
  %13 = load i32, ptr %6, align 4, !tbaa !11
  %14 = sext i32 %13 to i64
  %15 = getelementptr inbounds float, ptr %12, i64 %14
  %16 = load float, ptr %15, align 4, !tbaa !14
  %17 = load float, ptr %5, align 4, !tbaa !14
  %18 = call float @llvm.fmuladd.f32(float %16, float 5.000000e-01, float %17)
  store float %18, ptr %5, align 4, !tbaa !14
  br label %19

19:                                               ; preds = %11
  %20 = load i32, ptr %6, align 4, !tbaa !11
  %21 = add nsw i32 %20, 1
  store i32 %21, ptr %6, align 4, !tbaa !11
  br label %7, !llvm.loop !22

22:                                               ; preds = %7
  %23 = load float, ptr %5, align 4, !tbaa !14
  ret float %23
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @cmp(double noundef %0, double noundef %1) #0 {
  %3 = alloca double, align 8
  %4 = alloca double, align 8
  store double %0, ptr %3, align 8, !tbaa !9
  store double %1, ptr %4, align 8, !tbaa !9
  %5 = load double, ptr %3, align 8, !tbaa !9
  %6 = load double, ptr %4, align 8, !tbaa !9
  %7 = fcmp olt double %5, %6
  br i1 %7, label %8, label %9

8:                                                ; preds = %2
  br label %15

9:                                                ; preds = %2
  %10 = load double, ptr %3, align 8, !tbaa !9
  %11 = load double, ptr %4, align 8, !tbaa !9
  %12 = fcmp ogt double %10, %11
  %13 = zext i1 %12 to i64
  %14 = select i1 %12, i32 1, i32 0
  br label %15

15:                                               ; preds = %9, %8
  %16 = phi i32 [ -1, %8 ], [ %14, %9 ]
  ret i32 %16
}

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare double @llvm.fmuladd.f64(double, double, double) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare float @llvm.fmuladd.f32(float, float, float) #2

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #2 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}
!llvm.errno.tbaa = !{!4}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"uwtable", i32 1}
!2 = !{i32 7, !"frame-pointer", i32 4}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!4 = !{!5, !6, i64 0}
!5 = !{!"__libc_errno", !6, i64 0}
!6 = !{!"int", !7, i64 0}
!7 = !{!"omnipotent char", !8, i64 0}
!8 = !{!"Simple C/C++ TBAA"}
!9 = !{!10, !10, i64 0}
!10 = !{!"double", !7, i64 0}
!11 = !{!6, !6, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
!14 = !{!15, !15, i64 0}
!15 = !{!"float", !7, i64 0}
!16 = distinct !{!16, !13}
!17 = !{!18, !18, i64 0}
!18 = !{!"long", !7, i64 0}
!19 = !{!20, !20, i64 0}
!20 = !{!"p1 float", !21, i64 0}
!21 = !{!"any pointer", !7, i64 0}
!22 = distinct !{!22, !13}
