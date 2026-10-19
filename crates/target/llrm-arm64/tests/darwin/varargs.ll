source_filename = "varargs.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@.str = private unnamed_addr constant [9 x i8] c"%ld %ld\0A\00", align 1
@.str.1 = private unnamed_addr constant [4 x i8] c"%d\0A\00", align 1
@.str.2 = private unnamed_addr constant [6 x i8] c"dsdsd\00", align 1
@.str.3 = private unnamed_addr constant [3 x i8] c"ab\00", align 1
@.str.4 = private unnamed_addr constant [3 x i8] c"cd\00", align 1
@.str.5 = private unnamed_addr constant [7 x i8] c"%d %s\0A\00", align 1

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca [64 x i8], align 1
  %3 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %4 = call i64 (i32, ...) @sum(i32 noundef 3, i32 noundef 1, i32 noundef 2, i32 noundef 3)
  %5 = call i64 (i32, ...) @sum(i32 noundef 10, i32 noundef 1, i32 noundef 2, i32 noundef 3, i32 noundef 4, i32 noundef 5, i32 noundef 6, i32 noundef 7, i32 noundef 8, i32 noundef 9, i32 noundef 10)
  %6 = call i32 (ptr, ...) @printf(ptr noundef @.str, i64 noundef %4, i64 noundef %5)
  %7 = call double (i32, ...) @mix(i32 noundef 3, double noundef 1.500000e+00, i64 noundef 10, double noundef 2.500000e+00, i64 noundef 20, double noundef 3.500000e+00, i64 noundef 30)
  %8 = fptosi double %7 to i32
  %9 = call i32 (ptr, ...) @printf(ptr noundef @.str.1, i32 noundef %8)
  %10 = getelementptr inbounds [64 x i8], ptr %2, i64 0, i64 0
  %11 = call i32 (ptr, ptr, ...) @fmt(ptr noundef %10, ptr noundef @.str.2, i32 noundef 4, ptr noundef @.str.3, i32 noundef 5, ptr noundef @.str.4, i32 noundef 6)
  store i32 %11, ptr %3, align 4, !tbaa !9
  %12 = load i32, ptr %3, align 4, !tbaa !9
  %13 = getelementptr inbounds [64 x i8], ptr %2, i64 0, i64 0
  %14 = call i32 (ptr, ...) @printf(ptr noundef @.str.5, i32 noundef %12, ptr noundef %13)
  ret i32 0
}

declare i32 @printf(ptr noundef, ...) #1

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @sum(i32 noundef %0, ...) #0 {
  %2 = alloca i32, align 4
  %3 = alloca ptr, align 8
  %4 = alloca i64, align 8
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !9
  call void @llvm.va_start.p0(ptr %3)
  store i64 0, ptr %4, align 8, !tbaa !10
  store i32 0, ptr %5, align 4, !tbaa !9
  br label %7

7:                                                ; preds = %17, %1
  %8 = load i32, ptr %5, align 4, !tbaa !9
  %9 = load i32, ptr %2, align 4, !tbaa !9
  %10 = icmp slt i32 %8, %9
  br i1 %10, label %11, label %20

11:                                               ; preds = %7
  %12 = va_arg ptr %3, i32
  store i32 %12, ptr %6, align 4
  %13 = load i32, ptr %6, align 4, !tbaa !9
  %14 = sext i32 %13 to i64
  %15 = load i64, ptr %4, align 8, !tbaa !10
  %16 = add nsw i64 %15, %14
  store i64 %16, ptr %4, align 8, !tbaa !10
  br label %17

17:                                               ; preds = %11
  %18 = load i32, ptr %5, align 4, !tbaa !9
  %19 = add nsw i32 %18, 1
  store i32 %19, ptr %5, align 4, !tbaa !9
  br label %7, !llvm.loop !12

20:                                               ; preds = %7
  call void @llvm.va_end.p0(ptr %3)
  %21 = load i64, ptr %4, align 8, !tbaa !10
  ret i64 %21
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal double @mix(i32 noundef %0, ...) #0 {
  %2 = alloca i32, align 4
  %3 = alloca ptr, align 8
  %4 = alloca double, align 8
  %5 = alloca i32, align 4
  %6 = alloca double, align 8
  %7 = alloca i64, align 8
  store i32 %0, ptr %2, align 4, !tbaa !9
  call void @llvm.va_start.p0(ptr %3)
  store double 0.000000e+00, ptr %4, align 8, !tbaa !14
  store i32 0, ptr %5, align 4, !tbaa !9
  br label %8

8:                                                ; preds = %22, %1
  %9 = load i32, ptr %5, align 4, !tbaa !9
  %10 = load i32, ptr %2, align 4, !tbaa !9
  %11 = icmp slt i32 %9, %10
  br i1 %11, label %12, label %25

12:                                               ; preds = %8
  %13 = va_arg ptr %3, double
  store double %13, ptr %6, align 8
  %14 = load double, ptr %6, align 8, !tbaa !14
  %15 = load double, ptr %4, align 8, !tbaa !14
  %16 = fadd double %15, %14
  store double %16, ptr %4, align 8, !tbaa !14
  %17 = va_arg ptr %3, i64
  store i64 %17, ptr %7, align 8
  %18 = load i64, ptr %7, align 8, !tbaa !10
  %19 = sitofp i64 %18 to double
  %20 = load double, ptr %4, align 8, !tbaa !14
  %21 = fadd double %20, %19
  store double %21, ptr %4, align 8, !tbaa !14
  br label %22

22:                                               ; preds = %12
  %23 = load i32, ptr %5, align 4, !tbaa !9
  %24 = add nsw i32 %23, 1
  store i32 %24, ptr %5, align 4, !tbaa !9
  br label %8, !llvm.loop !16

25:                                               ; preds = %8
  call void @llvm.va_end.p0(ptr %3)
  %26 = load double, ptr %4, align 8, !tbaa !14
  ret double %26
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @fmt(ptr noundef %0, ptr noundef %1, ...) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca ptr, align 8
  %5 = alloca ptr, align 8
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  %8 = alloca i32, align 4
  %9 = alloca ptr, align 8
  %10 = alloca ptr, align 8
  store ptr %0, ptr %3, align 8, !tbaa !17
  store ptr %1, ptr %4, align 8, !tbaa !17
  call void @llvm.va_start.p0(ptr %5)
  store i32 0, ptr %6, align 4, !tbaa !9
  br label %11

11:                                               ; preds = %56, %2
  %12 = load ptr, ptr %4, align 8, !tbaa !17
  %13 = load i8, ptr %12, align 1, !tbaa !20
  %14 = icmp ne i8 %13, 0
  br i1 %14, label %15, label %59

15:                                               ; preds = %11
  %16 = load ptr, ptr %4, align 8, !tbaa !17
  %17 = load i8, ptr %16, align 1, !tbaa !20
  %18 = sext i8 %17 to i32
  %19 = icmp eq i32 %18, 100
  br i1 %19, label %20, label %32

20:                                               ; preds = %15
  %21 = va_arg ptr %5, i32
  store i32 %21, ptr %8, align 4
  %22 = load i32, ptr %8, align 4, !tbaa !9
  store i32 %22, ptr %7, align 4, !tbaa !9
  %23 = load i32, ptr %7, align 4, !tbaa !9
  %24 = srem i32 %23, 10
  %25 = add nsw i32 48, %24
  %26 = trunc i32 %25 to i8
  %27 = load ptr, ptr %3, align 8, !tbaa !17
  %28 = load i32, ptr %6, align 4, !tbaa !9
  %29 = add nsw i32 %28, 1
  store i32 %29, ptr %6, align 4, !tbaa !9
  %30 = sext i32 %28 to i64
  %31 = getelementptr inbounds i8, ptr %27, i64 %30
  store i8 %26, ptr %31, align 1, !tbaa !20
  br label %55

32:                                               ; preds = %15
  %33 = load ptr, ptr %4, align 8, !tbaa !17
  %34 = load i8, ptr %33, align 1, !tbaa !20
  %35 = sext i8 %34 to i32
  %36 = icmp eq i32 %35, 115
  br i1 %36, label %37, label %54

37:                                               ; preds = %32
  %38 = va_arg ptr %5, ptr
  store ptr %38, ptr %10, align 8
  %39 = load ptr, ptr %10, align 8, !tbaa !17
  store ptr %39, ptr %9, align 8, !tbaa !17
  br label %40

40:                                               ; preds = %44, %37
  %41 = load ptr, ptr %9, align 8, !tbaa !17
  %42 = load i8, ptr %41, align 1, !tbaa !20
  %43 = icmp ne i8 %42, 0
  br i1 %43, label %44, label %53

44:                                               ; preds = %40
  %45 = load ptr, ptr %9, align 8, !tbaa !17
  %46 = getelementptr inbounds nuw i8, ptr %45, i32 1
  store ptr %46, ptr %9, align 8, !tbaa !17
  %47 = load i8, ptr %45, align 1, !tbaa !20
  %48 = load ptr, ptr %3, align 8, !tbaa !17
  %49 = load i32, ptr %6, align 4, !tbaa !9
  %50 = add nsw i32 %49, 1
  store i32 %50, ptr %6, align 4, !tbaa !9
  %51 = sext i32 %49 to i64
  %52 = getelementptr inbounds i8, ptr %48, i64 %51
  store i8 %47, ptr %52, align 1, !tbaa !20
  br label %40, !llvm.loop !21

53:                                               ; preds = %40
  br label %54

54:                                               ; preds = %53, %32
  br label %55

55:                                               ; preds = %54, %20
  br label %56

56:                                               ; preds = %55
  %57 = load ptr, ptr %4, align 8, !tbaa !17
  %58 = getelementptr inbounds nuw i8, ptr %57, i32 1
  store ptr %58, ptr %4, align 8, !tbaa !17
  br label %11, !llvm.loop !22

59:                                               ; preds = %11
  %60 = load ptr, ptr %3, align 8, !tbaa !17
  %61 = load i32, ptr %6, align 4, !tbaa !9
  %62 = sext i32 %61 to i64
  %63 = getelementptr inbounds i8, ptr %60, i64 %62
  store i8 0, ptr %63, align 1, !tbaa !20
  call void @llvm.va_end.p0(ptr %5)
  %64 = load i32, ptr %6, align 4, !tbaa !9
  ret i32 %64
}

; Function Attrs: nocallback nofree nosync nounwind willreturn
declare void @llvm.va_start.p0(ptr) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn
declare void @llvm.va_end.p0(ptr) #2

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #2 = { nocallback nofree nosync nounwind willreturn }

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
!10 = !{!11, !11, i64 0}
!11 = !{!"long", !7, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
!14 = !{!15, !15, i64 0}
!15 = !{!"double", !7, i64 0}
!16 = distinct !{!16, !13}
!17 = !{!18, !18, i64 0}
!18 = !{!"p1 omnipotent char", !19, i64 0}
!19 = !{!"any pointer", !7, i64 0}
!20 = !{!7, !7, i64 0}
!21 = distinct !{!21, !13}
!22 = distinct !{!22, !13}
