target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

%struct.big = type { [10 x i32] }
%struct.odd = type { [41 x i8] }

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %1 = alloca %struct.odd, align 1
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  br label %2

2:
  %3 = phi i8 [ %8, %2 ], [ 0, %0 ]
  %4 = phi ptr [ %7, %2 ], [ %1, %0 ]
  %5 = phi i32 [ %6, %2 ], [ 41, %0 ]
  store i8 %3, ptr %4, align 1, !tbaa !13
  %6 = add nsw i32 %5, -1
  %7 = getelementptr i8, ptr %4, i32 1
  %8 = add nuw nsw i8 %3, 1
  %exitcond1 = icmp eq i32 %6, 0
  br i1 %exitcond1, label %9, label %2, !llvm.loop !14

9:
  tail call void @report(i32 noundef 254) nobuiltin "no-builtins"
  tail call void @report(i32 noundef 1) nobuiltin "no-builtins"
  %10 = getelementptr inbounds nuw i8, ptr %1, i32 40
  %11 = load i8, ptr %10, !tbaa !13
  %12 = sext i8 %11 to i32
  %13 = add nsw i32 %12, 1161
  tail call void @report(i32 noundef %13) nobuiltin "no-builtins"
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret i32 0
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.memcpy.p0.p0.i32(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i32, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

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
!9 = !{!10, !10, i64 0}
!10 = !{!"long", !6, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = !{!6, !6, i64 0}
!14 = distinct !{!14, !12}
!15 = distinct !{!15, !12}
