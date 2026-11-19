target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define double @sum(i32 noundef %0, ...) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" nofree nosync norecurse willreturn {
  %2 = alloca ptr, align 2
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.va_start.p0(ptr nonnull %2)
  %3 = icmp sgt i32 %0, 0
  br i1 %3, label %4, label %11

4:
  br label %5

5:
  %6 = phi i32 [ %0, %4 ], [ %10, %5 ]
  %7 = phi double [ %9, %5 ], [ 0.000000e+00, %4 ]
  %8 = va_arg ptr %2, double
  %9 = fadd double %7, %8
  %10 = add i32 %6, -1
  %exitcond = icmp eq i32 %10, 0
  br i1 %exitcond, label %11, label %5, !llvm.loop !12

11:
  %12 = phi double [ 0.000000e+00, %1 ], [ %9, %5 ]
  call void @llvm.va_end.p0(ptr nonnull %2)
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  ret double %12
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.va_start.p0(ptr) nocallback nofree nosync nounwind willreturn

declare void @llvm.va_end.p0(ptr) nocallback nofree nosync nounwind willreturn

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %1 = alloca double, align 8
  %2 = alloca double, align 8
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %1) nounwind
  store volatile double 1.500000e+00, ptr %1, align 8
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %2) nounwind
  store volatile double 2.500000e-01, ptr %2, align 8
  %3 = load volatile double, ptr %1, align 8, !tbaa !10
  %4 = load volatile double, ptr %2, align 8, !tbaa !10
  %5 = load volatile double, ptr %1, align 8, !tbaa !10
  %6 = tail call double (i32, ...) @sum(i32 noundef 3, double noundef %3, double noundef %4, double noundef %5) nobuiltin "no-builtins"
  %7 = fmul double %6, 4.000000e+00
  %8 = fptosi double %7 to i32
  tail call void @report(i32 noundef %8) nobuiltin "no-builtins"
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %1) nounwind
  ret i32 0
}

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}
!llvm.errno.tbaa = !{!4}

!0 = !{i32 1, !"long-double-type", !"double"}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"PIE Level", i32 2}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!4 = !{!5, !6, i64 0}
!5 = !{!"__libc_errno", !6, i64 0}
!6 = !{!"int", !7, i64 0}
!7 = !{!"omnipotent char", !8, i64 0}
!8 = !{!"Simple C/C++ TBAA"}
!9 = !{!6, !6, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"long double", !7, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
