; ModuleID = 'bench/mandel/mandel.c'
source_filename = "bench/mandel/mandel.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

; Function Attrs: nofree norecurse nosync nounwind memory(none)
define dso_local i32 @bench_mandel() local_unnamed_addr #0 {
  br label %1

1:                                                ; preds = %0, %38
  %2 = phi i32 [ -12, %0 ], [ %39, %38 ]
  %3 = phi i32 [ 0, %0 ], [ %35, %38 ]
  %4 = mul nsw i32 %2, 24
  br label %5

5:                                                ; preds = %1, %32
  %6 = phi i32 [ -16, %1 ], [ %36, %32 ]
  %7 = phi i32 [ %3, %1 ], [ %35, %32 ]
  %8 = mul nsw i32 %6, 24
  %9 = add nsw i32 %8, -128
  br label %10

10:                                               ; preds = %5, %29
  %11 = phi i32 [ 0, %5 ], [ %28, %29 ]
  %12 = phi i32 [ 0, %5 ], [ %27, %29 ]
  %13 = phi i16 [ 0, %5 ], [ %30, %29 ]
  %14 = mul nsw i32 %12, %12
  %15 = lshr i32 %14, 8
  %16 = mul nsw i32 %11, %11
  %17 = lshr i32 %16, 8
  %18 = add nuw nsw i32 %17, %15
  %19 = icmp samesign ult i32 %18, 1025
  br i1 %19, label %20, label %26

20:                                               ; preds = %10
  %21 = mul nsw i32 %11, %12
  %22 = ashr i32 %21, 7
  %23 = add nsw i32 %22, %4
  %24 = add nsw i32 %9, %15
  %25 = sub nsw i32 %24, %17
  br label %26

26:                                               ; preds = %10, %20
  %27 = phi i32 [ %25, %20 ], [ %12, %10 ]
  %28 = phi i32 [ %23, %20 ], [ %11, %10 ]
  br i1 %19, label %29, label %32

29:                                               ; preds = %26
  %30 = add nuw nsw i16 %13, 1
  %31 = icmp eq i16 %30, 32
  br i1 %31, label %32, label %10, !llvm.loop !8

32:                                               ; preds = %26, %29
  %33 = phi i16 [ %13, %26 ], [ 32, %29 ]
  %34 = zext nneg i16 %33 to i32
  %35 = add nsw i32 %7, %34
  %36 = add nsw i32 %6, 1
  %37 = icmp eq i32 %36, 16
  br i1 %37, label %38, label %5, !llvm.loop !11

38:                                               ; preds = %32
  %39 = add nsw i32 %2, 1
  %40 = icmp eq i32 %39, 12
  br i1 %40, label %41, label %1, !llvm.loop !12

41:                                               ; preds = %38
  ret i32 %35
}

; Function Attrs: nounwind
define dso_local noundef i32 @main() local_unnamed_addr #1 {
  br label %1

1:                                                ; preds = %38, %0
  %2 = phi i32 [ -12, %0 ], [ %39, %38 ]
  %3 = phi i32 [ 0, %0 ], [ %35, %38 ]
  %4 = mul nsw i32 %2, 24
  br label %5

5:                                                ; preds = %32, %1
  %6 = phi i32 [ -16, %1 ], [ %36, %32 ]
  %7 = phi i32 [ %3, %1 ], [ %35, %32 ]
  %8 = mul nsw i32 %6, 24
  %9 = add nsw i32 %8, -128
  br label %10

10:                                               ; preds = %29, %5
  %11 = phi i32 [ 0, %5 ], [ %28, %29 ]
  %12 = phi i32 [ 0, %5 ], [ %27, %29 ]
  %13 = phi i16 [ 0, %5 ], [ %30, %29 ]
  %14 = mul nsw i32 %12, %12
  %15 = lshr i32 %14, 8
  %16 = mul nsw i32 %11, %11
  %17 = lshr i32 %16, 8
  %18 = add nuw nsw i32 %15, %17
  %19 = icmp samesign ult i32 %18, 1025
  br i1 %19, label %20, label %26

20:                                               ; preds = %10
  %21 = mul nsw i32 %12, %11
  %22 = ashr i32 %21, 7
  %23 = add nsw i32 %22, %4
  %24 = sub i32 %9, %17
  %25 = add i32 %24, %15
  br label %26

26:                                               ; preds = %20, %10
  %27 = phi i32 [ %25, %20 ], [ %12, %10 ]
  %28 = phi i32 [ %23, %20 ], [ %11, %10 ]
  br i1 %19, label %29, label %32

29:                                               ; preds = %26
  %30 = add nuw nsw i16 %13, 1
  %31 = icmp eq i16 %30, 32
  br i1 %31, label %32, label %10, !llvm.loop !8

32:                                               ; preds = %29, %26
  %33 = phi i16 [ %13, %26 ], [ 32, %29 ]
  %34 = zext nneg i16 %33 to i32
  %35 = add nsw i32 %7, %34
  %36 = add nsw i32 %6, 1
  %37 = icmp eq i32 %36, 16
  br i1 %37, label %38, label %5, !llvm.loop !11

38:                                               ; preds = %32
  %39 = add nsw i32 %2, 1
  %40 = icmp eq i32 %39, 12
  br i1 %40, label %41, label %1, !llvm.loop !12

41:                                               ; preds = %38
  tail call void @report(i32 noundef %35) #3
  ret i32 0
}

declare void @report(i32 noundef) local_unnamed_addr #2

attributes #0 = { nofree norecurse nosync nounwind memory(none) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #1 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #2 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #3 = { nobuiltin nounwind "no-builtins" }

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.errno.tbaa = !{!3}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!3 = !{!4, !5, i64 0}
!4 = !{!"__libc_errno", !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
!8 = distinct !{!8, !9, !10}
!9 = !{!"llvm.loop.mustprogress"}
!10 = !{!"llvm.loop.unroll.disable"}
!11 = distinct !{!11, !9, !10}
!12 = distinct !{!12, !9, !10}
