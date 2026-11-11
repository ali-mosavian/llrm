target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

; Function Attrs: nofree norecurse nosync nounwind memory(argmem: read)
define dso_local i32 @k(ptr nofree noundef readonly captures(none) %0, i32 noundef %1, i32 noundef %2) local_unnamed_addr #0 {
  %4 = add nsw i32 %1, -1
  br label %5

5:                                                ; preds = %22, %3
  %6 = phi i32 [ undef, %3 ], [ %23, %22 ]
  %7 = phi i32 [ 0, %3 ], [ %24, %22 ]
  %8 = phi i32 [ %4, %3 ], [ %25, %22 ]
  %9 = icmp sgt i32 %7, %8
  br i1 %9, label %26, label %10

10:                                               ; preds = %5
  %11 = add nuw nsw i32 %8, %7
  %12 = lshr i32 %11, 1
  %13 = getelementptr inbounds nuw [4 x i8], ptr %0, i32 %12
  %14 = load i32, ptr %13, align 2, !tbaa !8
  %15 = icmp eq i32 %14, %2
  br i1 %15, label %22, label %16

16:                                               ; preds = %10
  %17 = icmp slt i32 %14, %2
  %18 = add nuw nsw i32 %12, 1
  %19 = add nsw i32 %12, -1
  %20 = select i1 %17, i32 %18, i32 %7
  %21 = select i1 %17, i32 %8, i32 %19
  br label %22

22:                                               ; preds = %10, %16
  %23 = phi i32 [ %6, %16 ], [ %12, %10 ]
  %24 = phi i32 [ %20, %16 ], [ %7, %10 ]
  %25 = phi i32 [ %21, %16 ], [ %8, %10 ]
  br i1 %15, label %26, label %5, !llvm.loop !9

26:                                               ; preds = %5, %22
  %27 = phi i32 [ %23, %22 ], [ -1, %5 ]
  ret i32 %27
}

attributes #0 = { nofree norecurse nosync nounwind memory(argmem: read) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }

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
!8 = !{!5, !5, i64 0}
!9 = distinct !{!9, !10, !11}
!10 = !{!"llvm.loop.mustprogress"}
!11 = !{!"llvm.loop.unroll.disable"}
