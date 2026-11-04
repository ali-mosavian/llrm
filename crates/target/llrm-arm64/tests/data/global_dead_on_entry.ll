; ModuleID = 'tests/run/c/float_in_memory_loop.c'
source_filename = "tests/run/c/float_in_memory_loop.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

@acc = internal global double 0.000000e+00, align 8
@step = internal global double 0.000000e+00, align 8

; Function Attrs: nounwind ssp
define i64 @sum_of_steps(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i64, align 8
  store i32 %0, ptr %2, align 4, !tbaa !9
  store i64 0, ptr %5, align 8, !tbaa !10
  store i32 0, ptr %4, align 4, !tbaa !9
  br label %6

6:                                                ; preds = %29, %1
  %7 = load i32, ptr %4, align 4, !tbaa !9
  %8 = load i32, ptr %2, align 4, !tbaa !9
  %9 = icmp slt i32 %7, %8
  br i1 %9, label %10, label %32

10:                                               ; preds = %6
  store double 0.000000e+00, ptr @acc, align 8, !tbaa !12
  store double 5.000000e-01, ptr @step, align 8, !tbaa !12
  store i32 0, ptr %3, align 4, !tbaa !9
  br label %11

11:                                               ; preds = %18, %10
  %12 = load i32, ptr %3, align 4, !tbaa !9
  %13 = icmp slt i32 %12, 16
  br i1 %13, label %14, label %21

14:                                               ; preds = %11
  %15 = load double, ptr @step, align 8, !tbaa !12
  %16 = load double, ptr @acc, align 8, !tbaa !12
  %17 = fadd double %16, %15
  store double %17, ptr @acc, align 8, !tbaa !12
  br label %18

18:                                               ; preds = %14
  %19 = load i32, ptr %3, align 4, !tbaa !9
  %20 = add nsw i32 %19, 1
  store i32 %20, ptr %3, align 4, !tbaa !9
  br label %11, !llvm.loop !14

21:                                               ; preds = %11
  %22 = load double, ptr @acc, align 8, !tbaa !12
  %23 = fptosi double %22 to i64
  %24 = load i32, ptr %4, align 4, !tbaa !9
  %25 = sext i32 %24 to i64
  %26 = add nsw i64 %23, %25
  %27 = load i64, ptr %5, align 8, !tbaa !10
  %28 = add nsw i64 %27, %26
  store i64 %28, ptr %5, align 8, !tbaa !10
  br label %29

29:                                               ; preds = %21
  %30 = load i32, ptr %4, align 4, !tbaa !9
  %31 = add nsw i32 %30, 1
  store i32 %31, ptr %4, align 4, !tbaa !9
  br label %6, !llvm.loop !16

32:                                               ; preds = %6
  %33 = load i64, ptr %5, align 8, !tbaa !10
  ret i64 %33
}

; Function Attrs: nounwind ssp
define i32 @main() #0 {
  %1 = call i64 @sum_of_steps(i32 noundef 100) #2
  call void @report(i64 noundef %1) #2
  ret i32 0
}

declare void @report(i64 noundef) #1

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
!11 = !{!"long", !7, i64 0}
!12 = !{!13, !13, i64 0}
!13 = !{!"double", !7, i64 0}
!14 = distinct !{!14, !15}
!15 = !{!"llvm.loop.mustprogress"}
!16 = distinct !{!16, !15}
