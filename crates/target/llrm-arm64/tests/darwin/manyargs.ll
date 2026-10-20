source_filename = "manyargs.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@.str = private unnamed_addr constant [5 x i8] c"%ld\0A\00", align 1
@.str.1 = private unnamed_addr constant [4 x i8] c"%d\0A\00", align 1
@.str.2 = private unnamed_addr constant [7 x i8] c"%d %d\0A\00", align 1
@.str.3 = private unnamed_addr constant [20 x i8] c"%d %ld %c %s %u %x\0A\00", align 1
@.str.4 = private unnamed_addr constant [4 x i8] c"str\00", align 1

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @main() #0 {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = call i64 @f(i64 noundef 1, i64 noundef 2, i64 noundef 3, i64 noundef 4, i64 noundef 5, i64 noundef 6, i64 noundef 7, i64 noundef 8, i64 noundef 9, i64 noundef 10, i32 noundef 11, i8 noundef signext 12)
  %3 = call i32 (ptr, ...) @printf(ptr noundef @.str, i64 noundef %2)
  %4 = call double @g(double noundef 1.000000e+00, double noundef 2.000000e+00, double noundef 3.000000e+00, double noundef 4.000000e+00, double noundef 5.000000e+00, double noundef 6.000000e+00, double noundef 7.000000e+00, double noundef 8.000000e+00, double noundef 9.000000e+00, double noundef 1.000000e+01)
  %5 = fptosi double %4 to i32
  %6 = call i32 (ptr, ...) @printf(ptr noundef @.str.1, i32 noundef %5)
  %7 = call i32 @apply(ptr noundef @plus, i32 noundef 6, i32 noundef 7)
  %8 = call i32 @apply(ptr noundef @times, i32 noundef 6, i32 noundef 7)
  %9 = call i32 (ptr, ...) @printf(ptr noundef @.str.2, i32 noundef %7, i32 noundef %8)
  %10 = call i32 (ptr, ...) @printf(ptr noundef @.str.3, i32 noundef -5, i64 noundef 1234567890123, i32 noundef 113, ptr noundef @.str.4, i32 noundef -294967296, i32 noundef 48879)
  ret i32 0
}

declare i32 @printf(ptr noundef, ...) #1

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @f(i64 noundef %0, i64 noundef %1, i64 noundef %2, i64 noundef %3, i64 noundef %4, i64 noundef %5, i64 noundef %6, i64 noundef %7, i64 noundef %8, i64 noundef %9, i32 noundef %10, i8 noundef signext %11) #0 {
  %13 = alloca i64, align 8
  %14 = alloca i64, align 8
  %15 = alloca i64, align 8
  %16 = alloca i64, align 8
  %17 = alloca i64, align 8
  %18 = alloca i64, align 8
  %19 = alloca i64, align 8
  %20 = alloca i64, align 8
  %21 = alloca i64, align 8
  %22 = alloca i64, align 8
  %23 = alloca i32, align 4
  %24 = alloca i8, align 1
  store i64 %0, ptr %13, align 8, !tbaa !9
  store i64 %1, ptr %14, align 8, !tbaa !9
  store i64 %2, ptr %15, align 8, !tbaa !9
  store i64 %3, ptr %16, align 8, !tbaa !9
  store i64 %4, ptr %17, align 8, !tbaa !9
  store i64 %5, ptr %18, align 8, !tbaa !9
  store i64 %6, ptr %19, align 8, !tbaa !9
  store i64 %7, ptr %20, align 8, !tbaa !9
  store i64 %8, ptr %21, align 8, !tbaa !9
  store i64 %9, ptr %22, align 8, !tbaa !9
  store i32 %10, ptr %23, align 4, !tbaa !11
  store i8 %11, ptr %24, align 1, !tbaa !12
  %25 = load i64, ptr %13, align 8, !tbaa !9
  %26 = load i64, ptr %14, align 8, !tbaa !9
  %27 = mul nsw i64 2, %26
  %28 = add nsw i64 %25, %27
  %29 = load i64, ptr %15, align 8, !tbaa !9
  %30 = mul nsw i64 3, %29
  %31 = add nsw i64 %28, %30
  %32 = load i64, ptr %16, align 8, !tbaa !9
  %33 = mul nsw i64 4, %32
  %34 = add nsw i64 %31, %33
  %35 = load i64, ptr %17, align 8, !tbaa !9
  %36 = mul nsw i64 5, %35
  %37 = add nsw i64 %34, %36
  %38 = load i64, ptr %18, align 8, !tbaa !9
  %39 = mul nsw i64 6, %38
  %40 = add nsw i64 %37, %39
  %41 = load i64, ptr %19, align 8, !tbaa !9
  %42 = mul nsw i64 7, %41
  %43 = add nsw i64 %40, %42
  %44 = load i64, ptr %20, align 8, !tbaa !9
  %45 = mul nsw i64 8, %44
  %46 = add nsw i64 %43, %45
  %47 = load i64, ptr %21, align 8, !tbaa !9
  %48 = mul nsw i64 9, %47
  %49 = add nsw i64 %46, %48
  %50 = load i64, ptr %22, align 8, !tbaa !9
  %51 = mul nsw i64 10, %50
  %52 = add nsw i64 %49, %51
  %53 = load i32, ptr %23, align 4, !tbaa !11
  %54 = mul nsw i32 11, %53
  %55 = sext i32 %54 to i64
  %56 = add nsw i64 %52, %55
  %57 = load i8, ptr %24, align 1, !tbaa !12
  %58 = sext i8 %57 to i32
  %59 = mul nsw i32 12, %58
  %60 = sext i32 %59 to i64
  %61 = add nsw i64 %56, %60
  ret i64 %61
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal double @g(double noundef %0, double noundef %1, double noundef %2, double noundef %3, double noundef %4, double noundef %5, double noundef %6, double noundef %7, double noundef %8, double noundef %9) #0 {
  %11 = alloca double, align 8
  %12 = alloca double, align 8
  %13 = alloca double, align 8
  %14 = alloca double, align 8
  %15 = alloca double, align 8
  %16 = alloca double, align 8
  %17 = alloca double, align 8
  %18 = alloca double, align 8
  %19 = alloca double, align 8
  %20 = alloca double, align 8
  store double %0, ptr %11, align 8, !tbaa !13
  store double %1, ptr %12, align 8, !tbaa !13
  store double %2, ptr %13, align 8, !tbaa !13
  store double %3, ptr %14, align 8, !tbaa !13
  store double %4, ptr %15, align 8, !tbaa !13
  store double %5, ptr %16, align 8, !tbaa !13
  store double %6, ptr %17, align 8, !tbaa !13
  store double %7, ptr %18, align 8, !tbaa !13
  store double %8, ptr %19, align 8, !tbaa !13
  store double %9, ptr %20, align 8, !tbaa !13
  %21 = load double, ptr %11, align 8, !tbaa !13
  %22 = load double, ptr %12, align 8, !tbaa !13
  %23 = call double @llvm.fmuladd.f64(double %22, double 2.000000e+00, double %21)
  %24 = load double, ptr %13, align 8, !tbaa !13
  %25 = call double @llvm.fmuladd.f64(double %24, double 3.000000e+00, double %23)
  %26 = load double, ptr %14, align 8, !tbaa !13
  %27 = call double @llvm.fmuladd.f64(double %26, double 4.000000e+00, double %25)
  %28 = load double, ptr %15, align 8, !tbaa !13
  %29 = call double @llvm.fmuladd.f64(double %28, double 5.000000e+00, double %27)
  %30 = load double, ptr %16, align 8, !tbaa !13
  %31 = call double @llvm.fmuladd.f64(double %30, double 6.000000e+00, double %29)
  %32 = load double, ptr %17, align 8, !tbaa !13
  %33 = call double @llvm.fmuladd.f64(double %32, double 7.000000e+00, double %31)
  %34 = load double, ptr %18, align 8, !tbaa !13
  %35 = call double @llvm.fmuladd.f64(double %34, double 8.000000e+00, double %33)
  %36 = load double, ptr %19, align 8, !tbaa !13
  %37 = call double @llvm.fmuladd.f64(double %36, double 9.000000e+00, double %35)
  %38 = load double, ptr %20, align 8, !tbaa !13
  %39 = call double @llvm.fmuladd.f64(double %38, double 1.000000e+01, double %37)
  ret double %39
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @apply(ptr noundef %0, i32 noundef %1, i32 noundef %2) #0 {
  %4 = alloca ptr, align 8
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  store ptr %0, ptr %4, align 8, !tbaa !15
  store i32 %1, ptr %5, align 4, !tbaa !11
  store i32 %2, ptr %6, align 4, !tbaa !11
  %7 = load ptr, ptr %4, align 8, !tbaa !15
  %8 = load i32, ptr %5, align 4, !tbaa !11
  %9 = load i32, ptr %6, align 4, !tbaa !11
  %10 = call i32 %7(i32 noundef %8, i32 noundef %9)
  ret i32 %10
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @plus(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !11
  store i32 %1, ptr %4, align 4, !tbaa !11
  %5 = load i32, ptr %3, align 4, !tbaa !11
  %6 = load i32, ptr %4, align 4, !tbaa !11
  %7 = add nsw i32 %5, %6
  ret i32 %7
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @times(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !11
  store i32 %1, ptr %4, align 4, !tbaa !11
  %5 = load i32, ptr %3, align 4, !tbaa !11
  %6 = load i32, ptr %4, align 4, !tbaa !11
  %7 = mul nsw i32 %5, %6
  ret i32 %7
}

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare double @llvm.fmuladd.f64(double, double, double) #2

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
!10 = !{!"long", !7, i64 0}
!11 = !{!6, !6, i64 0}
!12 = !{!7, !7, i64 0}
!13 = !{!14, !14, i64 0}
!14 = !{!"double", !7, i64 0}
!15 = !{!16, !16, i64 0}
!16 = !{!"any pointer", !7, i64 0}
