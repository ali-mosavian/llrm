target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  br label %1

1:
  %2 = phi i32 [ %37, %1 ], [ 0, %0 ]
  %3 = phi i64 [ 0, %0 ], [ %36, %1 ]
  %4 = add i32 %2, -1
  %5 = icmp eq i32 %2, 1
  %6 = mul nsw i64 %3, 3
  %7 = icmp slt i32 %4, -1
  %8 = zext i1 %7 to i64
  %9 = add nsw i64 %6, %8
  %10 = icmp eq i32 %2, 0
  %11 = select i1 %10, i64 0, i64 2
  %12 = add nsw i64 %9, %11
  %13 = select i1 %5, i64 0, i64 5
  %14 = sext i1 %10 to i64
  %15 = add i64 %12, %14
  %16 = add i64 %15, %13
  %17 = mul nsw i64 %16, 3
  %18 = lshr i32 %4, 31
  %19 = zext nneg i32 %18 to i64
  %20 = add nsw i64 %17, %19
  %21 = icmp eq i32 %2, 1
  %22 = select i1 %21, i64 0, i64 2
  %23 = add nsw i64 %20, %22
  %24 = sext i1 %21 to i64
  %25 = add i64 %23, %24
  %26 = mul nsw i64 %25, 3
  %27 = icmp slt i32 %4, 1
  %28 = zext i1 %27 to i64
  %29 = add nsw i64 %26, %28
  %30 = icmp eq i32 %2, 2
  %31 = select i1 %30, i64 0, i64 2
  %32 = add nsw i64 %29, %31
  %33 = select i1 %5, i64 0, i64 5
  %34 = sext i1 %30 to i64
  %35 = add i64 %32, %34
  %36 = add i64 %35, %33
  %37 = add nuw nsw i32 %2, 1
  %exitcond1 = icmp eq i32 %37, 3
  br i1 %exitcond1, label %38, label %1, !llvm.loop !13

38:
  %39 = trunc i64 %36 to i32
  tail call void @report(i32 noundef %39) nobuiltin "no-builtins"
  %40 = lshr i64 %36, 3
  %41 = trunc i64 %40 to i32
  %42 = add nsw i32 %41, 2
  tail call void @report(i32 noundef %42) nobuiltin "no-builtins"
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
!9 = !{!"long long", !6, i64 0}
!10 = !{!5, !5, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = distinct !{!13, !12}
!14 = !{!15, !15, i64 0}
!15 = !{!"long", !6, i64 0}
