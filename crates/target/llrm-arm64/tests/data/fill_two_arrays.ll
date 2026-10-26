; ModuleID = 'tests/llc/fsum.c'
source_filename = "tests/llc/fsum.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

@.str = private unnamed_addr constant [9 x i8] c"%ld %ld\0A\00", align 1

; Function Attrs: nounwind ssp
define i32 @main() #0 {
  %1 = alloca [16 x double], align 8
  %2 = alloca [16 x float], align 4
  %3 = alloca i32, align 4
  store i32 0, ptr %3, align 4, !tbaa !9
  br label %4

4:                                                ; preds = %22, %0
  %5 = load i32, ptr %3, align 4, !tbaa !9
  %6 = icmp slt i32 %5, 16
  br i1 %6, label %7, label %25

7:                                                ; preds = %4
  %8 = load i32, ptr %3, align 4, !tbaa !9
  %9 = sitofp i32 %8 to double
  %10 = fmul double %9, 5.000000e-01
  %11 = fadd double %10, 1.000000e+00
  %12 = load i32, ptr %3, align 4, !tbaa !9
  %13 = sext i32 %12 to i64
  %14 = getelementptr inbounds [16 x double], ptr %1, i64 0, i64 %13
  store double %11, ptr %14, align 8, !tbaa !10
  %15 = load i32, ptr %3, align 4, !tbaa !9
  %16 = sitofp i32 %15 to float
  %17 = fmul float %16, 2.500000e-01
  %18 = fsub float %17, 2.000000e+00
  %19 = load i32, ptr %3, align 4, !tbaa !9
  %20 = sext i32 %19 to i64
  %21 = getelementptr inbounds [16 x float], ptr %2, i64 0, i64 %20
  store float %18, ptr %21, align 4, !tbaa !12
  br label %22

22:                                               ; preds = %7
  %23 = load i32, ptr %3, align 4, !tbaa !9
  %24 = add nsw i32 %23, 1
  store i32 %24, ptr %3, align 4, !tbaa !9
  br label %4, !llvm.loop !14

25:                                               ; preds = %4
  %26 = getelementptr inbounds [16 x double], ptr %1, i64 0, i64 0
  %27 = call double @dsum(ptr noundef %26, i32 noundef 16) #2
  %28 = fmul double %27, 1.000000e+03
  %29 = fptosi double %28 to i64
  %30 = getelementptr inbounds [16 x float], ptr %2, i64 0, i64 0
  %31 = call float @fsum(ptr noundef %30, i32 noundef 16) #2
  %32 = fmul float %31, 1.000000e+03
  %33 = fptosi float %32 to i64
  %34 = call i32 (ptr, ...) @printf(ptr noundef @.str, i64 noundef %29, i64 noundef %33) #2
  ret i32 0
}

declare i32 @printf(ptr noundef, ...) #1

; Function Attrs: nounwind ssp
define internal double @dsum(ptr noundef %0, i32 noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca i32, align 4
  %5 = alloca double, align 8
  %6 = alloca i32, align 4
  store ptr %0, ptr %3, align 8, !tbaa !16
  store i32 %1, ptr %4, align 4, !tbaa !9
  store double 0.000000e+00, ptr %5, align 8, !tbaa !10
  store i32 0, ptr %6, align 4, !tbaa !9
  br label %7

7:                                                ; preds = %19, %2
  %8 = load i32, ptr %6, align 4, !tbaa !9
  %9 = load i32, ptr %4, align 4, !tbaa !9
  %10 = icmp slt i32 %8, %9
  br i1 %10, label %11, label %22

11:                                               ; preds = %7
  %12 = load ptr, ptr %3, align 8, !tbaa !16
  %13 = load i32, ptr %6, align 4, !tbaa !9
  %14 = sext i32 %13 to i64
  %15 = getelementptr inbounds double, ptr %12, i64 %14
  %16 = load double, ptr %15, align 8, !tbaa !10
  %17 = load double, ptr %5, align 8, !tbaa !10
  %18 = fadd double %17, %16
  store double %18, ptr %5, align 8, !tbaa !10
  br label %19

19:                                               ; preds = %11
  %20 = load i32, ptr %6, align 4, !tbaa !9
  %21 = add nsw i32 %20, 1
  store i32 %21, ptr %6, align 4, !tbaa !9
  br label %7, !llvm.loop !19

22:                                               ; preds = %7
  %23 = load double, ptr %5, align 8, !tbaa !10
  ret double %23
}

; Function Attrs: nounwind ssp
define internal float @fsum(ptr noundef %0, i32 noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca i32, align 4
  %5 = alloca float, align 4
  %6 = alloca i32, align 4
  store ptr %0, ptr %3, align 8, !tbaa !20
  store i32 %1, ptr %4, align 4, !tbaa !9
  store float 0.000000e+00, ptr %5, align 4, !tbaa !12
  store i32 0, ptr %6, align 4, !tbaa !9
  br label %7

7:                                                ; preds = %19, %2
  %8 = load i32, ptr %6, align 4, !tbaa !9
  %9 = load i32, ptr %4, align 4, !tbaa !9
  %10 = icmp slt i32 %8, %9
  br i1 %10, label %11, label %22

11:                                               ; preds = %7
  %12 = load ptr, ptr %3, align 8, !tbaa !20
  %13 = load i32, ptr %6, align 4, !tbaa !9
  %14 = sext i32 %13 to i64
  %15 = getelementptr inbounds float, ptr %12, i64 %14
  %16 = load float, ptr %15, align 4, !tbaa !12
  %17 = load float, ptr %5, align 4, !tbaa !12
  %18 = fadd float %17, %16
  store float %18, ptr %5, align 4, !tbaa !12
  br label %19

19:                                               ; preds = %11
  %20 = load i32, ptr %6, align 4, !tbaa !9
  %21 = add nsw i32 %20, 1
  store i32 %21, ptr %6, align 4, !tbaa !9
  br label %7, !llvm.loop !22

22:                                               ; preds = %7
  %23 = load float, ptr %5, align 4, !tbaa !12
  ret float %23
}

attributes #0 = { nounwind ssp "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #2 = { nobuiltin "no-builtins" }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}
!llvm.errno.tbaa = !{!4}

!0 = !{i32 2, !"SDK Version", [2 x i32] [i32 27, i32 0]}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"frame-pointer", i32 4}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!4 = !{!5, !6, i64 0}
!5 = !{!"__libc_errno", !6, i64 0}
!6 = !{!"int", !7, i64 0}
!7 = !{!"omnipotent char", !8, i64 0}
!8 = !{!"Simple C/C++ TBAA"}
!9 = !{!6, !6, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"double", !7, i64 0}
!12 = !{!13, !13, i64 0}
!13 = !{!"float", !7, i64 0}
!14 = distinct !{!14, !15}
!15 = !{!"llvm.loop.mustprogress"}
!16 = !{!17, !17, i64 0}
!17 = !{!"p1 double", !18, i64 0}
!18 = !{!"any pointer", !7, i64 0}
!19 = distinct !{!19, !15}
!20 = !{!21, !21, i64 0}
!21 = !{!"p1 float", !18, i64 0}
!22 = distinct !{!22, !15}
