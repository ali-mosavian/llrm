; ModuleID = '/private/tmp/claude-501/-Users-alim-work/24f055cb-2e50-4e3d-a501-a06bc7ded045/scratchpad/at/u.c'
source_filename = "/private/tmp/claude-501/-Users-alim-work/24f055cb-2e50-4e3d-a501-a06bc7ded045/scratchpad/at/u.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

@n = global i32 0, align 4
@l = global i64 0, align 8

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @add() #0 {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  store i32 1, ptr %1, align 4, !tbaa !10
  %3 = load i32, ptr %1, align 4
  %4 = atomicrmw add ptr @n, i32 %3 seq_cst, align 4
  store i32 %4, ptr %2, align 4
  %5 = load i32, ptr %2, align 4, !tbaa !10
  ret i32 %5
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @cas(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  %5 = alloca i8, align 1
  store i64 %0, ptr %2, align 8, !tbaa !11
  %6 = load i64, ptr %2, align 8, !tbaa !11
  store i64 %6, ptr %3, align 8, !tbaa !11
  store i64 5, ptr %4, align 8, !tbaa !11
  %7 = load i64, ptr %3, align 8
  %8 = load i64, ptr %4, align 8
  %9 = cmpxchg ptr @l, i64 %7, i64 %8 seq_cst seq_cst, align 8
  %10 = extractvalue { i64, i1 } %9, 0
  %11 = extractvalue { i64, i1 } %9, 1
  br i1 %11, label %13, label %12

12:                                               ; preds = %1
  store i64 %10, ptr %3, align 8
  br label %13

13:                                               ; preds = %12, %1
  %14 = zext i1 %11 to i8
  store i8 %14, ptr %5, align 1, !tbaa !13
  %15 = load i8, ptr %5, align 1, !tbaa !13, !range !15, !noundef !16
  %16 = trunc i8 %15 to i1
  %17 = zext i1 %16 to i32
  ret i32 %17
}

; Function Attrs: nounwind ssp uwtable(sync)
define void @loop() #0 {
  %1 = alloca i64, align 8
  %2 = alloca i64, align 8
  %3 = alloca i64, align 8
  %4 = alloca i8, align 1
  %5 = load atomic i64, ptr @l seq_cst, align 8
  store i64 %5, ptr %2, align 8
  %6 = load i64, ptr %2, align 8, !tbaa !11
  store i64 %6, ptr %1, align 8, !tbaa !11
  br label %7

7:                                                ; preds = %21, %0
  %8 = load i64, ptr %1, align 8, !tbaa !11
  %9 = add nsw i64 %8, 2
  store i64 %9, ptr %3, align 8, !tbaa !11
  %10 = load i64, ptr %1, align 8
  %11 = load i64, ptr %3, align 8
  %12 = cmpxchg weak ptr @l, i64 %10, i64 %11 seq_cst seq_cst, align 8
  %13 = extractvalue { i64, i1 } %12, 0
  %14 = extractvalue { i64, i1 } %12, 1
  br i1 %14, label %16, label %15

15:                                               ; preds = %7
  store i64 %13, ptr %1, align 8
  br label %16

16:                                               ; preds = %15, %7
  %17 = zext i1 %14 to i8
  store i8 %17, ptr %4, align 1, !tbaa !13
  %18 = load i8, ptr %4, align 1, !tbaa !13, !range !15, !noundef !16
  %19 = trunc i8 %18 to i1
  %20 = xor i1 %19, true
  br i1 %20, label %21, label %22

21:                                               ; preds = %16
  br label %7, !llvm.loop !17

22:                                               ; preds = %16
  ret void
}

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }

!llvm.module.flags = !{!0, !1, !2, !3}
!llvm.ident = !{!4}
!llvm.errno.tbaa = !{!5}

!0 = !{i32 2, !"SDK Version", [2 x i32] [i32 27, i32 0]}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"uwtable", i32 1}
!3 = !{i32 7, !"frame-pointer", i32 4}
!4 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!5 = !{!6, !7, i64 0}
!6 = !{!"__libc_errno", !7, i64 0}
!7 = !{!"int", !8, i64 0}
!8 = !{!"omnipotent char", !9, i64 0}
!9 = !{!"Simple C/C++ TBAA"}
!10 = !{!7, !7, i64 0}
!11 = !{!12, !12, i64 0}
!12 = !{!"long", !8, i64 0}
!13 = !{!14, !14, i64 0}
!14 = !{!"_Bool", !8, i64 0}
!15 = !{i8 0, i8 2}
!16 = !{}
!17 = distinct !{!17, !18}
!18 = !{!"llvm.loop.mustprogress"}
