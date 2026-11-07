; ModuleID = '/home/alim/scratch/wt-remat/crates/target/llrm-6502/tests/bench/hand/strncpy.c'
source_filename = "/home/alim/scratch/wt-remat/crates/target/llrm-6502/tests/bench/hand/strncpy.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "mos"

; Function Attrs: nofree norecurse nosync nounwind memory(argmem: readwrite)
define dso_local noundef ptr @k_strncpy(ptr nofree noundef returned writeonly captures(ret: address, provenance) %0, ptr nofree noundef readonly captures(none) %1, i16 noundef %2) local_unnamed_addr #0 {
  %4 = icmp eq i16 %2, 0
  br i1 %4, label %17, label %5

5:                                                ; preds = %3, %11
  %6 = phi ptr [ %12, %11 ], [ %0, %3 ]
  %7 = phi i16 [ %14, %11 ], [ %2, %3 ]
  %8 = phi ptr [ %13, %11 ], [ %1, %3 ]
  %9 = load i8, ptr %8, align 1, !tbaa !7
  store i8 %9, ptr %6, align 1, !tbaa !7
  %10 = icmp eq i8 %9, 0
  br i1 %10, label %16, label %11

11:                                               ; preds = %5
  %12 = getelementptr inbounds nuw i8, ptr %6, i16 1
  %13 = getelementptr inbounds nuw i8, ptr %8, i16 1
  %14 = add i16 %7, -1
  %15 = icmp eq i16 %14, 0
  br i1 %15, label %17, label %5, !llvm.loop !8

16:                                               ; preds = %5
  tail call void @llvm.memset.p0.i16(ptr noundef nonnull align 1 dereferenceable(1) %6, i8 0, i16 %7, i1 false), !tbaa !7
  br label %17

17:                                               ; preds = %11, %3, %16
  ret ptr %0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: write)
declare void @llvm.memset.p0.i16(ptr writeonly captures(none), i8, i16, i1 immarg) #1

attributes #0 = { nofree norecurse nosync nounwind memory(argmem: readwrite) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: write) }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!5, !5, i64 0}
!8 = distinct !{!8, !9}
!9 = !{!"llvm.loop.mustprogress"}
