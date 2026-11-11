; ModuleID = '/home/alim/scratch/mw-run/tmp/ov1.c'
source_filename = "/home/alim/scratch/mw-run/tmp/ov1.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

@sa = dso_local global [3 x i16] [i16 32000, i16 -32000, i16 7], align 2

; Function Attrs: nounwind
define dso_local noundef i32 @main() local_unnamed_addr #0 {
  br label %1

1:                                                ; preds = %0, %23
  %2 = phi i32 [ 0, %0 ], [ %24, %23 ]
  %3 = getelementptr inbounds nuw [2 x i8], ptr @sa, i32 %2
  br label %4

4:                                                ; preds = %1, %4
  %5 = phi i32 [ 0, %1 ], [ %21, %4 ]
  %6 = load volatile i16, ptr %3, align 2, !tbaa !8
  %7 = getelementptr inbounds nuw [2 x i8], ptr @sa, i32 %5
  %8 = load volatile i16, ptr %7, align 2, !tbaa !8
  %9 = tail call { i16, i1 } @llvm.sadd.with.overflow.i16(i16 %6, i16 %8)
  %10 = extractvalue { i16, i1 } %9, 1
  %11 = extractvalue { i16, i1 } %9, 0
  %12 = zext i1 %10 to i32
  tail call void @report(i32 noundef %12) #3
  %13 = sext i16 %11 to i32
  tail call void @report(i32 noundef %13) #3
  %14 = load volatile i16, ptr %3, align 2, !tbaa !8
  %15 = load volatile i16, ptr %7, align 2, !tbaa !8
  %16 = tail call { i16, i1 } @llvm.ssub.with.overflow.i16(i16 %14, i16 %15)
  %17 = extractvalue { i16, i1 } %16, 1
  %18 = extractvalue { i16, i1 } %16, 0
  %19 = zext i1 %17 to i32
  tail call void @report(i32 noundef %19) #3
  %20 = sext i16 %18 to i32
  tail call void @report(i32 noundef %20) #3
  %21 = add nuw nsw i32 %5, 1
  %22 = icmp eq i32 %21, 3
  br i1 %22, label %23, label %4, !llvm.loop !10

23:                                               ; preds = %4
  %24 = add nuw nsw i32 %2, 1
  %25 = icmp eq i32 %24, 3
  br i1 %25, label %26, label %1, !llvm.loop !13

26:                                               ; preds = %23
  ret i32 0
}

; Function Attrs: mustprogress nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i16, i1 } @llvm.sadd.with.overflow.i16(i16, i16) #1

declare void @report(i32 noundef) local_unnamed_addr #2

; Function Attrs: mustprogress nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i16, i1 } @llvm.ssub.with.overflow.i16(i16, i16) #1

attributes #0 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #1 = { mustprogress nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
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
!8 = !{!9, !9, i64 0}
!9 = !{!"short", !6, i64 0}
!10 = distinct !{!10, !11, !12}
!11 = !{!"llvm.loop.mustprogress"}
!12 = !{!"llvm.loop.unroll.disable"}
!13 = distinct !{!13, !11, !12}
