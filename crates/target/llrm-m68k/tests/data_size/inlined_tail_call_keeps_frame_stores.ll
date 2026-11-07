target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @run() noinline nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" willreturn nofree nosync norecurse memory(none) {
  %1 = alloca [2 x i32], align 2
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call fastcc void @set(ptr noundef %1) nobuiltin "no-builtins"
  %2 = load i32, ptr %1, align 2, !tbaa !8
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret i32 %2
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %1 = tail call i32 @run() nobuiltin "no-builtins"
  tail call void @report(i32 noundef %1) nobuiltin "no-builtins"
  ret i32 0
}

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

define internal fastcc void @set(ptr noundef nonnull nocapture nofree writeonly initializes((0, 4)) "llrm.reads"="" "llrm.writes"="0:4" %0) noinline nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" willreturn nofree nosync norecurse memory(argmem: write) {
  store i32 5, ptr %0, align 2, !tbaa !8
  ret void
}

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
!10 = !{!"p1 int", !11, i64 0}
!11 = !{!"any pointer", !6, i64 0}
