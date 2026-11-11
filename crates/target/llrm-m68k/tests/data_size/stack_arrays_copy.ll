target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %1 = alloca [70 x i8], align 1
  %2 = alloca [70 x i8], align 1
  %3 = alloca [70 x i8], align 1
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %3) nounwind
  br label %4

4:
  %5 = phi i8 [ %10, %4 ], [ 0, %0 ]
  %6 = phi ptr [ %9, %4 ], [ %2, %0 ]
  %7 = phi i32 [ %8, %4 ], [ 70, %0 ]
  store i8 %5, ptr %6, align 1, !tbaa !9
  %8 = add nsw i32 %7, -1
  %9 = getelementptr i8, ptr %6, i32 1
  %10 = add nuw nsw i8 %5, 1
  %exitcond2 = icmp eq i32 %8, 0
  br i1 %exitcond2, label %11, label %4, !llvm.loop !10

11:
  br label %12

12:
  %13 = phi ptr [ %2, %11 ], [ %19, %12 ]
  %14 = phi ptr [ %3, %11 ], [ %18, %12 ]
  %15 = phi i32 [ 63, %11 ], [ %17, %12 ]
  %16 = load i8, ptr %13, align 1, !tbaa !9
  store i8 %16, ptr %14, align 1, !tbaa !9
  %17 = add nsw i32 %15, -1
  %18 = getelementptr i8, ptr %14, i32 1
  %exitcond11 = icmp eq i32 %17, 0
  %19 = getelementptr i8, ptr %13, i32 1
  br i1 %exitcond11, label %20, label %12, !llvm.loop !12

20:
  %21 = getelementptr inbounds nuw i8, ptr %3, i32 3
  %22 = load i8, ptr %21, align 1, !tbaa !9
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %3) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  br label %23

23:
  %24 = phi i8 [ %29, %23 ], [ 0, %20 ]
  %25 = phi ptr [ %28, %23 ], [ %1, %20 ]
  %26 = phi i32 [ %27, %23 ], [ 70, %20 ]
  store i8 %24, ptr %25, align 1, !tbaa !9
  %27 = add nsw i32 %26, -1
  %28 = getelementptr i8, ptr %25, i32 1
  %29 = add nuw nsw i8 %24, 1
  %exitcond = icmp eq i32 %27, 0
  br i1 %exitcond, label %30, label %23, !llvm.loop !10

30:
  %31 = getelementptr inbounds nuw i8, ptr %1, i32 3
  %32 = load i8, ptr %31, align 1, !tbaa !9
  %33 = sext i8 %22 to i32
  %34 = mul nsw i32 %33, 100
  %35 = sext i8 %32 to i32
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  %36 = add nsw i32 %34, %35
  tail call void @report(i32 noundef %36) nobuiltin "no-builtins"
  ret i32 0
}

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

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
!9 = !{!6, !6, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
