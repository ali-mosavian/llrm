target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

@v = internal global [14 x i32] [i32 0, i32 1, i32 2, i32 32767, i32 32768, i32 65535, i32 65536, i32 65537, i32 2147483647, i32 -2147483648, i32 -2147483647, i32 -65536, i32 -2, i32 -1], align 2

define i32 @main() nounwind optsize "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  br label %1

1:
  %2 = phi i32 [ 0, %0 ], [ %42, %41 ]
  %3 = getelementptr inbounds nuw [4 x i8], ptr @v, i32 %2
  br label %4

4:
  %5 = phi ptr [ %40, %4 ], [ @v, %1 ]
  %6 = phi i32 [ %39, %4 ], [ 14, %1 ]
  %7 = phi i32 [ 0, %1 ], [ %38, %4 ]
  %8 = mul i32 %7, 257
  %9 = load volatile i32, ptr %3, align 2, !tbaa !8
  %10 = load volatile i32, ptr %5, align 2, !tbaa !8
  %11 = icmp ult i32 %9, %10
  %12 = zext i1 %11 to i32
  %13 = icmp ugt i32 %9, %10
  %14 = select i1 %13, i32 4, i32 2
  %15 = or disjoint i32 %14, %12
  %16 = or disjoint i32 %15, 8
  %17 = select i1 %11, i32 %15, i32 %16
  %18 = icmp slt i32 %9, %10
  %19 = icmp sgt i32 %9, %10
  %20 = select i1 %18, i32 48, i32 32
  %21 = select i1 %19, i32 0, i32 %20
  %22 = or disjoint i32 %17, %21
  %23 = or disjoint i32 %22, 64
  %24 = select i1 %19, i32 %23, i32 %22
  %25 = add nuw nsw i32 %24, 128
  %26 = select i1 %18, i32 %22, i32 %25
  %27 = load volatile i32, ptr %3, align 2, !tbaa !8
  %28 = load volatile i32, ptr %5, align 2, !tbaa !8
  %29 = icmp ult i32 %27, %28
  %30 = select i1 %29, i32 179, i32 117
  %31 = icmp slt i32 %27, %28
  %32 = select i1 %31, i32 3328, i32 4352
  %33 = or disjoint i32 %30, %32
  %34 = select i1 %31, i32 94208, i32 77824
  %35 = add nuw nsw i32 %33, %34
  %36 = mul i32 %35, 65599
  %37 = add i32 %36, %8
  %38 = add i32 %37, %26
  %39 = add nsw i32 %6, -1
  %exitcond = icmp eq i32 %39, 0
  %40 = getelementptr i8, ptr %5, i32 4
  br i1 %exitcond, label %41, label %4, !llvm.loop !9

41:
  tail call void @report(i32 noundef %38) nobuiltin optsize "no-builtins"
  %42 = add nuw nsw i32 %2, 1
  %exitcond1 = icmp eq i32 %42, 14
  br i1 %exitcond1, label %43, label %1, !llvm.loop !11

43:
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
!8 = !{!5, !5, i64 0}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
!11 = distinct !{!11, !10}
