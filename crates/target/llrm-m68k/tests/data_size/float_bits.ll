target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

%union.D = type { double }
%union.F = type { float }

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %1 = alloca double, align 8
  tail call void @report(i32 noundef 0) nobuiltin "no-builtins"
  tail call void @report(i32 noundef 1073217536) nobuiltin "no-builtins"
  tail call void @report(i32 noundef 0) nobuiltin "no-builtins"
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %1) nounwind
  %sroa_idx2 = getelementptr inbounds nuw i8, ptr %1, i32 4
  store i32 1074790400, ptr %sroa_idx2, align 4
  store i32 0, ptr %1, align 8
  %2 = load double, ptr %1, align 8, !tbaa !10
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %1) nounwind
  %3 = fmul double %2, 3.000000e+00
  %4 = fptosi double %3 to i32
  tail call void @report(i32 noundef %4) nobuiltin "no-builtins"
  tail call void @report(i32 noundef 4161536) nobuiltin "no-builtins"
  tail call void @report(i32 noundef 20) nobuiltin "no-builtins"
  tail call void @report(i32 noundef -7) nobuiltin "no-builtins"
  ret i32 0
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

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
!10 = !{!6, !6, i64 0}
!11 = !{!5, !5, i64 0}
!12 = !{!13, !13, i64 0}
!13 = !{!"float", !6, i64 0}
