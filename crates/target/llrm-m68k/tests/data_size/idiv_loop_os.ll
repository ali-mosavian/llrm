target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @total(i32 noundef %0) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" memory(none) nofree nosync norecurse {
  %2 = icmp slt i32 %0, 1
  br i1 %2, label %28, label %3

3:
  br label %4

4:
  %5 = phi i32 [ %26, %23 ], [ 1, %3 ]
  %6 = phi i32 [ %25, %23 ], [ 0, %3 ]
  %7 = mul nuw nsw i32 %5, 7
  %8 = add nuw nsw i32 %7, 3
  %9 = mul nuw nsw i32 %5, 5
  %10 = add nuw nsw i32 %9, 2
  br label %11

11:
  %12 = phi i32 [ %14, %11 ], [ %10, %4 ]
  %13 = phi i32 [ %12, %11 ], [ %8, %4 ]
  %14 = srem i32 %13, %12
  %15 = icmp eq i32 %14, 0
  br i1 %15, label %16, label %11

16:
  %17 = add nuw nsw i32 %5, 90
  br label %18

18:
  %19 = phi i32 [ %21, %18 ], [ 360, %16 ]
  %20 = phi i32 [ %19, %18 ], [ %17, %16 ]
  %21 = srem i32 %20, %19
  %22 = icmp eq i32 %21, 0
  br i1 %22, label %23, label %18

23:
  %24 = add i32 %12, %6
  %25 = add i32 %24, %19
  %26 = add nuw i32 %5, 1
  %27 = icmp eq i32 %5, %0
  br i1 %27, label %28, label %4, !llvm.loop !11

28:
  %29 = phi i32 [ 0, %1 ], [ %25, %23 ]
  ret i32 %29
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  br label %1

1:
  %2 = phi i32 [ %23, %20 ], [ 1, %0 ]
  %3 = phi i32 [ %22, %20 ], [ 0, %0 ]
  %4 = mul nuw nsw i32 %2, 7
  %5 = add nuw nsw i32 %4, 3
  %6 = mul nuw nsw i32 %2, 5
  %7 = add nuw nsw i32 %6, 2
  br label %8

8:
  %9 = phi i32 [ %11, %8 ], [ %7, %1 ]
  %10 = phi i32 [ %9, %8 ], [ %5, %1 ]
  %11 = srem i32 %10, %9
  %12 = icmp eq i32 %11, 0
  br i1 %12, label %13, label %8

13:
  %14 = add nuw nsw i32 %2, 90
  br label %15

15:
  %16 = phi i32 [ %18, %15 ], [ 360, %13 ]
  %17 = phi i32 [ %16, %15 ], [ %14, %13 ]
  %18 = srem i32 %17, %16
  %19 = icmp eq i32 %18, 0
  br i1 %19, label %20, label %15

20:
  %21 = add i32 %9, %3
  %22 = add i32 %21, %16
  %23 = add nuw nsw i32 %2, 1
  %24 = icmp eq i32 %2, 300
  br i1 %24, label %25, label %1, !llvm.loop !11

25:
  tail call void @report(i32 noundef %22) nobuiltin "no-builtins"
  ret i32 0
}

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

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
