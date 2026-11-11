; ModuleID = 'zinit.c'
source_filename = "zinit.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

; Function Attrs: nounwind uwtable
define dso_local i32 @f(i32 noundef %0) local_unnamed_addr #0 {
  %2 = icmp sgt i32 %0, 0
  br i1 %2, label %19, label %17

3:                                                ; preds = %19
  %4 = add nsw i32 %39, %36
  %5 = add nsw i32 %4, %42
  %6 = add nsw i32 %5, %45
  %7 = add nsw i32 %6, %48
  %8 = add nsw i32 %7, %51
  %9 = add nsw i32 %8, %54
  %10 = add nsw i32 %9, %57
  %11 = add nsw i32 %10, %60
  %12 = add nsw i32 %11, %63
  %13 = add nsw i32 %12, %66
  %14 = add nsw i32 %13, %69
  %15 = add nsw i32 %14, %72
  %16 = add nsw i32 %15, %75
  br label %17

17:                                               ; preds = %3, %1
  %18 = phi i32 [ 0, %1 ], [ %16, %3 ]
  ret i32 %18

19:                                               ; preds = %1, %19
  %20 = phi i32 [ %76, %19 ], [ 0, %1 ]
  %21 = phi i32 [ %75, %19 ], [ 0, %1 ]
  %22 = phi i32 [ %72, %19 ], [ 0, %1 ]
  %23 = phi i32 [ %69, %19 ], [ 0, %1 ]
  %24 = phi i32 [ %66, %19 ], [ 0, %1 ]
  %25 = phi i32 [ %63, %19 ], [ 0, %1 ]
  %26 = phi i32 [ %60, %19 ], [ 0, %1 ]
  %27 = phi i32 [ %57, %19 ], [ 0, %1 ]
  %28 = phi i32 [ %54, %19 ], [ 0, %1 ]
  %29 = phi i32 [ %51, %19 ], [ 0, %1 ]
  %30 = phi i32 [ %48, %19 ], [ 0, %1 ]
  %31 = phi i32 [ %45, %19 ], [ 0, %1 ]
  %32 = phi i32 [ %42, %19 ], [ 0, %1 ]
  %33 = phi i32 [ %39, %19 ], [ 0, %1 ]
  %34 = phi i32 [ %36, %19 ], [ 0, %1 ]
  %35 = tail call i32 @next() #2
  %36 = add nsw i32 %35, %34
  %37 = tail call i32 @next() #2
  %38 = shl nsw i32 %37, 1
  %39 = add nsw i32 %38, %33
  %40 = tail call i32 @next() #2
  %41 = mul nsw i32 %40, 3
  %42 = add nsw i32 %41, %32
  %43 = tail call i32 @next() #2
  %44 = mul nsw i32 %43, 5
  %45 = add nsw i32 %44, %31
  %46 = tail call i32 @next() #2
  %47 = mul nsw i32 %46, 7
  %48 = add nsw i32 %47, %30
  %49 = tail call i32 @next() #2
  %50 = mul nsw i32 %49, 11
  %51 = add nsw i32 %50, %29
  %52 = tail call i32 @next() #2
  %53 = mul nsw i32 %52, 13
  %54 = add nsw i32 %53, %28
  %55 = tail call i32 @next() #2
  %56 = mul nsw i32 %55, 17
  %57 = add nsw i32 %56, %27
  %58 = tail call i32 @next() #2
  %59 = mul nsw i32 %58, 19
  %60 = add nsw i32 %59, %26
  %61 = tail call i32 @next() #2
  %62 = mul nsw i32 %61, 23
  %63 = add nsw i32 %62, %25
  %64 = tail call i32 @next() #2
  %65 = mul nsw i32 %64, 29
  %66 = add nsw i32 %65, %24
  %67 = tail call i32 @next() #2
  %68 = mul nsw i32 %67, 31
  %69 = add nsw i32 %68, %23
  %70 = tail call i32 @next() #2
  %71 = mul nsw i32 %70, 37
  %72 = add nsw i32 %71, %22
  %73 = tail call i32 @next() #2
  %74 = mul nsw i32 %73, 41
  %75 = add nsw i32 %74, %21
  %76 = add nuw nsw i32 %20, 1
  %77 = icmp eq i32 %76, %0
  br i1 %77, label %3, label %19, !llvm.loop !16
}

declare i32 @next() local_unnamed_addr #1

attributes #0 = { nounwind uwtable "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #2 = { nounwind }

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6, !7, !8, !9}
!llvm.ident = !{!10}
!llvm.errno.tbaa = !{!11}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 8, !"PIC Level", i32 2}
!7 = !{i32 7, !"PIE Level", i32 2}
!8 = !{i32 7, !"uwtable", i32 2}
!9 = !{i32 7, !"frame-pointer", i32 4}
!10 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!11 = !{!12, !13, i64 0}
!12 = !{!"__libc_errno", !13, i64 0}
!13 = !{!"int", !14, i64 0}
!14 = !{!"omnipotent char", !15, i64 0}
!15 = !{!"Simple C/C++ TBAA"}
!16 = distinct !{!16, !17, !18}
!17 = !{!"llvm.loop.mustprogress"}
!18 = !{!"llvm.loop.unroll.disable"}
