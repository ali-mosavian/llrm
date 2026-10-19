source_filename = "numeric.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@.str = private unnamed_addr constant [3 x i8] c"%c\00", align 1
@.str.1 = private unnamed_addr constant [11 x i8] c" .:-=+*#%@\00", align 1
@.str.2 = private unnamed_addr constant [2 x i8] c"\0A\00", align 1
@.str.3 = private unnamed_addr constant [7 x i8] c"%d %d\0A\00", align 1
@__const.main.a = private unnamed_addr constant [3 x [3 x double]] [[3 x double] [double 1.000000e+00, double 2.000000e+00, double 3.000000e+00], [3 x double] [double 4.000000e+00, double 5.000000e+00, double 6.000000e+00], [3 x double] [double 7.000000e+00, double 8.000000e+00, double 9.000000e+00]], align 8
@__const.main.b = private unnamed_addr constant [3 x [3 x double]] [[3 x double] [double 9.000000e+00, double 8.000000e+00, double 7.000000e+00], [3 x double] [double 6.000000e+00, double 5.000000e+00, double 4.000000e+00], [3 x double] [double 3.000000e+00, double 2.000000e+00, double 1.000000e+00]], align 8
@.str.4 = private unnamed_addr constant [10 x i8] c"%d %d %d\0A\00", align 1
@__const.main.data = private unnamed_addr constant [44 x i8] c"The quick brown fox jumps over the lazy dog\00", align 1
@.str.5 = private unnamed_addr constant [4 x i8] c"%x\0A\00", align 1

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca [3 x [3 x double]], align 8
  %5 = alloca [3 x [3 x double]], align 8
  %6 = alloca [3 x [3 x double]], align 8
  %7 = alloca i32, align 4
  %8 = alloca [44 x i8], align 1
  store i32 0, ptr %1, align 4
  store i32 -6, ptr %2, align 4, !tbaa !9
  br label %9

9:                                                ; preds = %36, %0
  %10 = load i32, ptr %2, align 4, !tbaa !9
  %11 = icmp sle i32 %10, 6
  br i1 %11, label %12, label %39

12:                                               ; preds = %9
  store i32 -10, ptr %3, align 4, !tbaa !9
  br label %13

13:                                               ; preds = %31, %12
  %14 = load i32, ptr %3, align 4, !tbaa !9
  %15 = icmp sle i32 %14, 4
  br i1 %15, label %16, label %34

16:                                               ; preds = %13
  %17 = load i32, ptr %3, align 4, !tbaa !9
  %18 = sitofp i32 %17 to double
  %19 = fdiv double %18, 5.000000e+00
  %20 = load i32, ptr %2, align 4, !tbaa !9
  %21 = sitofp i32 %20 to double
  %22 = fdiv double %21, 5.000000e+00
  %23 = call double @mandel(double noundef %19, double noundef %22)
  %24 = fptosi double %23 to i32
  %25 = srem i32 %24, 10
  %26 = sext i32 %25 to i64
  %27 = getelementptr inbounds [11 x i8], ptr @.str.1, i64 0, i64 %26
  %28 = load i8, ptr %27, align 1, !tbaa !10
  %29 = sext i8 %28 to i32
  %30 = call i32 (ptr, ...) @printf(ptr noundef @.str, i32 noundef %29)
  br label %31

31:                                               ; preds = %16
  %32 = load i32, ptr %3, align 4, !tbaa !9
  %33 = add nsw i32 %32, 2
  store i32 %33, ptr %3, align 4, !tbaa !9
  br label %13, !llvm.loop !11

34:                                               ; preds = %13
  %35 = call i32 (ptr, ...) @printf(ptr noundef @.str.2)
  br label %36

36:                                               ; preds = %34
  %37 = load i32, ptr %2, align 4, !tbaa !9
  %38 = add nsw i32 %37, 3
  store i32 %38, ptr %2, align 4, !tbaa !9
  br label %9, !llvm.loop !13

39:                                               ; preds = %9
  %40 = call double @sqrt_newton(double noundef 2.000000e+00)
  %41 = fmul double %40, 1.000000e+06
  %42 = fptosi double %41 to i32
  %43 = call double @sqrt_newton(double noundef 1.000000e+01)
  %44 = fmul double %43, 1.000000e+03
  %45 = fptosi double %44 to i32
  %46 = call i32 (ptr, ...) @printf(ptr noundef @.str.3, i32 noundef %42, i32 noundef %45)
  call void @llvm.memcpy.p0.p0.i64(ptr align 8 %4, ptr align 8 @__const.main.a, i64 72, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr align 8 %5, ptr align 8 @__const.main.b, i64 72, i1 false)
  %47 = getelementptr inbounds [3 x [3 x double]], ptr %4, i64 0, i64 0
  %48 = getelementptr inbounds [3 x [3 x double]], ptr %5, i64 0, i64 0
  %49 = getelementptr inbounds [3 x [3 x double]], ptr %6, i64 0, i64 0
  call void @matmul(ptr noundef %47, ptr noundef %48, ptr noundef %49)
  store i32 0, ptr %7, align 4, !tbaa !9
  br label %50

50:                                               ; preds = %73, %39
  %51 = load i32, ptr %7, align 4, !tbaa !9
  %52 = icmp slt i32 %51, 3
  br i1 %52, label %53, label %76

53:                                               ; preds = %50
  %54 = load i32, ptr %7, align 4, !tbaa !9
  %55 = sext i32 %54 to i64
  %56 = getelementptr inbounds [3 x [3 x double]], ptr %6, i64 0, i64 %55
  %57 = getelementptr inbounds [3 x double], ptr %56, i64 0, i64 0
  %58 = load double, ptr %57, align 8, !tbaa !14
  %59 = fptosi double %58 to i32
  %60 = load i32, ptr %7, align 4, !tbaa !9
  %61 = sext i32 %60 to i64
  %62 = getelementptr inbounds [3 x [3 x double]], ptr %6, i64 0, i64 %61
  %63 = getelementptr inbounds [3 x double], ptr %62, i64 0, i64 1
  %64 = load double, ptr %63, align 8, !tbaa !14
  %65 = fptosi double %64 to i32
  %66 = load i32, ptr %7, align 4, !tbaa !9
  %67 = sext i32 %66 to i64
  %68 = getelementptr inbounds [3 x [3 x double]], ptr %6, i64 0, i64 %67
  %69 = getelementptr inbounds [3 x double], ptr %68, i64 0, i64 2
  %70 = load double, ptr %69, align 8, !tbaa !14
  %71 = fptosi double %70 to i32
  %72 = call i32 (ptr, ...) @printf(ptr noundef @.str.4, i32 noundef %59, i32 noundef %65, i32 noundef %71)
  br label %73

73:                                               ; preds = %53
  %74 = load i32, ptr %7, align 4, !tbaa !9
  %75 = add nsw i32 %74, 1
  store i32 %75, ptr %7, align 4, !tbaa !9
  br label %50, !llvm.loop !16

76:                                               ; preds = %50
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %8, ptr align 1 @__const.main.data, i64 44, i1 false)
  %77 = getelementptr inbounds [44 x i8], ptr %8, i64 0, i64 0
  %78 = call i32 @crc32(ptr noundef %77, i32 noundef 43)
  %79 = call i32 (ptr, ...) @printf(ptr noundef @.str.5, i32 noundef %78)
  ret i32 0
}

declare i32 @printf(ptr noundef, ...) #1

; Function Attrs: nounwind ssp uwtable(sync)
define internal double @mandel(double noundef %0, double noundef %1) #0 {
  %3 = alloca double, align 8
  %4 = alloca double, align 8
  %5 = alloca double, align 8
  %6 = alloca double, align 8
  %7 = alloca i32, align 4
  %8 = alloca double, align 8
  store double %0, ptr %3, align 8, !tbaa !14
  store double %1, ptr %4, align 8, !tbaa !14
  store double 0.000000e+00, ptr %5, align 8, !tbaa !14
  store double 0.000000e+00, ptr %6, align 8, !tbaa !14
  store i32 0, ptr %7, align 4, !tbaa !9
  br label %9

9:                                                ; preds = %38, %2
  %10 = load i32, ptr %7, align 4, !tbaa !9
  %11 = icmp slt i32 %10, 50
  br i1 %11, label %12, label %20

12:                                               ; preds = %9
  %13 = load double, ptr %5, align 8, !tbaa !14
  %14 = load double, ptr %5, align 8, !tbaa !14
  %15 = load double, ptr %6, align 8, !tbaa !14
  %16 = load double, ptr %6, align 8, !tbaa !14
  %17 = fmul double %15, %16
  %18 = call double @llvm.fmuladd.f64(double %13, double %14, double %17)
  %19 = fcmp olt double %18, 4.000000e+00
  br label %20

20:                                               ; preds = %12, %9
  %21 = phi i1 [ false, %9 ], [ %19, %12 ]
  br i1 %21, label %22, label %41

22:                                               ; preds = %20
  %23 = load double, ptr %5, align 8, !tbaa !14
  %24 = load double, ptr %5, align 8, !tbaa !14
  %25 = load double, ptr %6, align 8, !tbaa !14
  %26 = load double, ptr %6, align 8, !tbaa !14
  %27 = fmul double %25, %26
  %28 = fneg double %27
  %29 = call double @llvm.fmuladd.f64(double %23, double %24, double %28)
  %30 = load double, ptr %3, align 8, !tbaa !14
  %31 = fadd double %29, %30
  store double %31, ptr %8, align 8, !tbaa !14
  %32 = load double, ptr %5, align 8, !tbaa !14
  %33 = fmul double 2.000000e+00, %32
  %34 = load double, ptr %6, align 8, !tbaa !14
  %35 = load double, ptr %4, align 8, !tbaa !14
  %36 = call double @llvm.fmuladd.f64(double %33, double %34, double %35)
  store double %36, ptr %6, align 8, !tbaa !14
  %37 = load double, ptr %8, align 8, !tbaa !14
  store double %37, ptr %5, align 8, !tbaa !14
  br label %38

38:                                               ; preds = %22
  %39 = load i32, ptr %7, align 4, !tbaa !9
  %40 = add nsw i32 %39, 1
  store i32 %40, ptr %7, align 4, !tbaa !9
  br label %9, !llvm.loop !17

41:                                               ; preds = %20
  %42 = load i32, ptr %7, align 4, !tbaa !9
  %43 = sitofp i32 %42 to double
  ret double %43
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal double @sqrt_newton(double noundef %0) #0 {
  %2 = alloca double, align 8
  %3 = alloca double, align 8
  %4 = alloca i32, align 4
  store double %0, ptr %2, align 8, !tbaa !14
  %5 = load double, ptr %2, align 8, !tbaa !14
  %6 = fdiv double %5, 2.000000e+00
  store double %6, ptr %3, align 8, !tbaa !14
  store i32 0, ptr %4, align 4, !tbaa !9
  br label %7

7:                                                ; preds = %17, %1
  %8 = load i32, ptr %4, align 4, !tbaa !9
  %9 = icmp slt i32 %8, 20
  br i1 %9, label %10, label %20

10:                                               ; preds = %7
  %11 = load double, ptr %3, align 8, !tbaa !14
  %12 = load double, ptr %2, align 8, !tbaa !14
  %13 = load double, ptr %3, align 8, !tbaa !14
  %14 = fdiv double %12, %13
  %15 = fadd double %11, %14
  %16 = fdiv double %15, 2.000000e+00
  store double %16, ptr %3, align 8, !tbaa !14
  br label %17

17:                                               ; preds = %10
  %18 = load i32, ptr %4, align 4, !tbaa !9
  %19 = add nsw i32 %18, 1
  store i32 %19, ptr %4, align 4, !tbaa !9
  br label %7, !llvm.loop !18

20:                                               ; preds = %7
  %21 = load double, ptr %3, align 8, !tbaa !14
  ret double %21
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i64, i1 immarg) #2

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @matmul(ptr noundef %0, ptr noundef %1, ptr noundef %2) #0 {
  %4 = alloca ptr, align 8
  %5 = alloca ptr, align 8
  %6 = alloca ptr, align 8
  %7 = alloca i32, align 4
  %8 = alloca i32, align 4
  %9 = alloca double, align 8
  %10 = alloca i32, align 4
  store ptr %0, ptr %4, align 8, !tbaa !19
  store ptr %1, ptr %5, align 8, !tbaa !19
  store ptr %2, ptr %6, align 8, !tbaa !19
  store i32 0, ptr %7, align 4, !tbaa !9
  br label %11

11:                                               ; preds = %57, %3
  %12 = load i32, ptr %7, align 4, !tbaa !9
  %13 = icmp slt i32 %12, 3
  br i1 %13, label %14, label %60

14:                                               ; preds = %11
  store i32 0, ptr %8, align 4, !tbaa !9
  br label %15

15:                                               ; preds = %53, %14
  %16 = load i32, ptr %8, align 4, !tbaa !9
  %17 = icmp slt i32 %16, 3
  br i1 %17, label %18, label %56

18:                                               ; preds = %15
  store double 0.000000e+00, ptr %9, align 8, !tbaa !14
  store i32 0, ptr %10, align 4, !tbaa !9
  br label %19

19:                                               ; preds = %41, %18
  %20 = load i32, ptr %10, align 4, !tbaa !9
  %21 = icmp slt i32 %20, 3
  br i1 %21, label %22, label %44

22:                                               ; preds = %19
  %23 = load ptr, ptr %4, align 8, !tbaa !19
  %24 = load i32, ptr %7, align 4, !tbaa !9
  %25 = sext i32 %24 to i64
  %26 = getelementptr inbounds [3 x double], ptr %23, i64 %25
  %27 = load i32, ptr %10, align 4, !tbaa !9
  %28 = sext i32 %27 to i64
  %29 = getelementptr inbounds [3 x double], ptr %26, i64 0, i64 %28
  %30 = load double, ptr %29, align 8, !tbaa !14
  %31 = load ptr, ptr %5, align 8, !tbaa !19
  %32 = load i32, ptr %10, align 4, !tbaa !9
  %33 = sext i32 %32 to i64
  %34 = getelementptr inbounds [3 x double], ptr %31, i64 %33
  %35 = load i32, ptr %8, align 4, !tbaa !9
  %36 = sext i32 %35 to i64
  %37 = getelementptr inbounds [3 x double], ptr %34, i64 0, i64 %36
  %38 = load double, ptr %37, align 8, !tbaa !14
  %39 = load double, ptr %9, align 8, !tbaa !14
  %40 = call double @llvm.fmuladd.f64(double %30, double %38, double %39)
  store double %40, ptr %9, align 8, !tbaa !14
  br label %41

41:                                               ; preds = %22
  %42 = load i32, ptr %10, align 4, !tbaa !9
  %43 = add nsw i32 %42, 1
  store i32 %43, ptr %10, align 4, !tbaa !9
  br label %19, !llvm.loop !22

44:                                               ; preds = %19
  %45 = load double, ptr %9, align 8, !tbaa !14
  %46 = load ptr, ptr %6, align 8, !tbaa !19
  %47 = load i32, ptr %7, align 4, !tbaa !9
  %48 = sext i32 %47 to i64
  %49 = getelementptr inbounds [3 x double], ptr %46, i64 %48
  %50 = load i32, ptr %8, align 4, !tbaa !9
  %51 = sext i32 %50 to i64
  %52 = getelementptr inbounds [3 x double], ptr %49, i64 0, i64 %51
  store double %45, ptr %52, align 8, !tbaa !14
  br label %53

53:                                               ; preds = %44
  %54 = load i32, ptr %8, align 4, !tbaa !9
  %55 = add nsw i32 %54, 1
  store i32 %55, ptr %8, align 4, !tbaa !9
  br label %15, !llvm.loop !23

56:                                               ; preds = %15
  br label %57

57:                                               ; preds = %56
  %58 = load i32, ptr %7, align 4, !tbaa !9
  %59 = add nsw i32 %58, 1
  store i32 %59, ptr %7, align 4, !tbaa !9
  br label %11, !llvm.loop !24

60:                                               ; preds = %11
  ret void
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @crc32(ptr noundef %0, i32 noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  store ptr %0, ptr %3, align 8, !tbaa !25
  store i32 %1, ptr %4, align 4, !tbaa !9
  store i32 -1, ptr %5, align 4, !tbaa !9
  store i32 0, ptr %6, align 4, !tbaa !9
  br label %8

8:                                                ; preds = %41, %2
  %9 = load i32, ptr %6, align 4, !tbaa !9
  %10 = load i32, ptr %4, align 4, !tbaa !9
  %11 = icmp slt i32 %9, %10
  br i1 %11, label %12, label %44

12:                                               ; preds = %8
  %13 = load ptr, ptr %3, align 8, !tbaa !25
  %14 = load i32, ptr %6, align 4, !tbaa !9
  %15 = sext i32 %14 to i64
  %16 = getelementptr inbounds i8, ptr %13, i64 %15
  %17 = load i8, ptr %16, align 1, !tbaa !10
  %18 = zext i8 %17 to i32
  %19 = load i32, ptr %5, align 4, !tbaa !9
  %20 = xor i32 %19, %18
  store i32 %20, ptr %5, align 4, !tbaa !9
  store i32 0, ptr %7, align 4, !tbaa !9
  br label %21

21:                                               ; preds = %37, %12
  %22 = load i32, ptr %7, align 4, !tbaa !9
  %23 = icmp slt i32 %22, 8
  br i1 %23, label %24, label %40

24:                                               ; preds = %21
  %25 = load i32, ptr %5, align 4, !tbaa !9
  %26 = and i32 %25, 1
  %27 = icmp ne i32 %26, 0
  br i1 %27, label %28, label %32

28:                                               ; preds = %24
  %29 = load i32, ptr %5, align 4, !tbaa !9
  %30 = lshr i32 %29, 1
  %31 = xor i32 %30, -306674912
  br label %35

32:                                               ; preds = %24
  %33 = load i32, ptr %5, align 4, !tbaa !9
  %34 = lshr i32 %33, 1
  br label %35

35:                                               ; preds = %32, %28
  %36 = phi i32 [ %31, %28 ], [ %34, %32 ]
  store i32 %36, ptr %5, align 4, !tbaa !9
  br label %37

37:                                               ; preds = %35
  %38 = load i32, ptr %7, align 4, !tbaa !9
  %39 = add nsw i32 %38, 1
  store i32 %39, ptr %7, align 4, !tbaa !9
  br label %21, !llvm.loop !27

40:                                               ; preds = %21
  br label %41

41:                                               ; preds = %40
  %42 = load i32, ptr %6, align 4, !tbaa !9
  %43 = add nsw i32 %42, 1
  store i32 %43, ptr %6, align 4, !tbaa !9
  br label %8, !llvm.loop !28

44:                                               ; preds = %8
  %45 = load i32, ptr %5, align 4, !tbaa !9
  %46 = xor i32 %45, -1
  ret i32 %46
}

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare double @llvm.fmuladd.f64(double, double, double) #3

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }

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
!9 = !{!6, !6, i64 0}
!10 = !{!7, !7, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = distinct !{!13, !12}
!14 = !{!15, !15, i64 0}
!15 = !{!"double", !7, i64 0}
!16 = distinct !{!16, !12}
!17 = distinct !{!17, !12}
!18 = distinct !{!18, !12}
!19 = !{!20, !20, i64 0}
!20 = !{!"p1 double", !21, i64 0}
!21 = !{!"any pointer", !7, i64 0}
!22 = distinct !{!22, !12}
!23 = distinct !{!23, !12}
!24 = distinct !{!24, !12}
!25 = !{!26, !26, i64 0}
!26 = !{!"p1 omnipotent char", !21, i64 0}
!27 = distinct !{!27, !12}
!28 = distinct !{!28, !12}
