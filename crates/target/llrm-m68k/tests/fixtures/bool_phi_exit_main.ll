target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @main() nounwind optsize "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  br label %1

1:
  %2 = phi double [ 1.000000e+00, %0 ], [ %13, %12 ]
  %3 = phi double [ 0.000000e+00, %0 ], [ %9, %12 ]
  %4 = phi double [ undef, %0 ], [ %7, %12 ]
  %5 = phi i1 [ true, %0 ], [ false, %12 ]
  %6 = phi i1 [ false, %0 ], [ true, %12 ]
  %7 = fmul double %2, %2
  %8 = fdiv double %7, %4
  %9 = select i1 %5, double %3, double %8
  %10 = fcmp oeq double %9, 1.000000e+00
  br i1 %10, label %11, label %12

11:
  tail call void @report(i32 noundef -1) nobuiltin optsize "no-builtins"
  br label %12

12:
  %13 = fmul double %2, 5.000000e-01
  br i1 %6, label %14, label %1, !llvm.loop !11

14:
  tail call void @report(i32 noundef 7) nobuiltin optsize "no-builtins"
  ret i32 0
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @report(i32 noundef) optsize "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

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
!9 = !{!"double", !6, i64 0}
!10 = !{!5, !5, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
