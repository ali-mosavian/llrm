; ModuleID = 'crates/target/llrm-m68k/bench/library/strcmp.c'
source_filename = "crates/target/llrm-m68k/bench/library/strcmp.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

; Function Attrs: nofree norecurse nosync nounwind memory(argmem: read)
define dso_local range(i32 -255, 256) i32 @k(ptr nofree noundef readonly captures(none) %0, ptr nofree noundef readonly captures(none) %1) local_unnamed_addr #0 {
  %3 = load i8, ptr %0, align 1, !tbaa !8
  %4 = icmp eq i8 %3, 0
  br i1 %4, label %20, label %5

5:                                                ; preds = %2, %11
  %6 = phi i8 [ %14, %11 ], [ %3, %2 ]
  %7 = phi ptr [ %13, %11 ], [ %1, %2 ]
  %8 = phi ptr [ %12, %11 ], [ %0, %2 ]
  %9 = load i8, ptr %7, align 1, !tbaa !8
  %10 = icmp eq i8 %6, %9
  br i1 %10, label %11, label %16

11:                                               ; preds = %5
  %12 = getelementptr inbounds nuw i8, ptr %8, i32 1
  %13 = getelementptr inbounds nuw i8, ptr %7, i32 1
  %14 = load i8, ptr %12, align 1, !tbaa !8
  %15 = icmp eq i8 %14, 0
  br i1 %15, label %16, label %5, !llvm.loop !9

16:                                               ; preds = %11, %5
  %17 = phi ptr [ %7, %5 ], [ %13, %11 ]
  %18 = phi i8 [ %6, %5 ], [ 0, %11 ]
  %19 = zext i8 %18 to i32
  br label %20

20:                                               ; preds = %16, %2
  %21 = phi ptr [ %1, %2 ], [ %17, %16 ]
  %22 = phi i32 [ 0, %2 ], [ %19, %16 ]
  %23 = load i8, ptr %21, align 1, !tbaa !8
  %24 = zext i8 %23 to i32
  %25 = sub nsw i32 %22, %24
  ret i32 %25
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
!8 = !{!6, !6, i64 0}
!9 = distinct !{!9, !10, !11}
!10 = !{!"llvm.loop.mustprogress"}
!11 = !{!"llvm.loop.unroll.disable"}
