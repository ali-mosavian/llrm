; ModuleID = '/home/alim/scratch/handoff/dagcopies/rp.c'
source_filename = "/home/alim/scratch/handoff/dagcopies/rp.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

; Function Attrs: nofree norecurse nosync nounwind ssp memory(none) uwtable(sync)
define double @f(double noundef %0, double noundef %1, i32 noundef %2) local_unnamed_addr #0 {
  %4 = icmp sgt i32 %2, 0
  br i1 %4, label %7, label %5

5:                                                ; preds = %7, %3
  %6 = phi double [ 0.000000e+00, %3 ], [ %12, %7 ]
  ret double %6

7:                                                ; preds = %3, %7
  %8 = phi i32 [ %14, %7 ], [ 0, %3 ]
  %9 = phi double [ %12, %7 ], [ 0.000000e+00, %3 ]
  %10 = phi double [ %13, %7 ], [ %0, %3 ]
  %11 = fmul double %1, %10
  %12 = fadd double %9, %11
  %13 = fmul double %10, 5.000000e-01
  %14 = add nuw nsw i32 %8, 1
  %15 = icmp eq i32 %14, %2
  br i1 %15, label %5, label %7, !llvm.loop !9
}

attributes #0 = { nofree norecurse nosync nounwind ssp memory(none) uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }

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
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
