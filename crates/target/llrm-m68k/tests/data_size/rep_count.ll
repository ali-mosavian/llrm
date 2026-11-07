target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

@g = global [16 x [54 x i8]] zeroinitializer, align 1

define i32 @bench() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" memory(readwrite, argmem: none, inaccessiblemem: none) nofree nosync norecurse willreturn {
  %1 = getelementptr i8, ptr @g, i32 54
  br label %2

2:
  %3 = phi ptr [ %11, %9 ], [ %1, %0 ]
  %4 = phi i32 [ 0, %0 ], [ %10, %9 ]
  br label %5

5:
  %6 = phi i32 [ %8, %5 ], [ -54, %2 ]
  %7 = getelementptr i8, ptr %3, i32 %6
  store i8 1, ptr %7, align 1, !tbaa !12
  %8 = add nsw i32 %6, 1
  %exitcond = icmp eq i32 %8, 0
  br i1 %exitcond, label %9, label %5, !llvm.loop !13

9:
  %10 = add nuw nsw i32 %4, 1
  %11 = getelementptr i8, ptr %3, i32 54
  %exitcond1 = icmp eq i32 %10, 16
  br i1 %exitcond1, label %12, label %2, !llvm.loop !15

12:
  %13 = getelementptr i8, ptr @g, i32 16
  br label %14

14:
  %15 = phi ptr [ %13, %12 ], [ %23, %21 ]
  %16 = phi i32 [ %22, %21 ], [ 0, %12 ]
  br label %17

17:
  %18 = phi i32 [ %20, %17 ], [ -16, %14 ]
  %19 = getelementptr i8, ptr %15, i32 %18
  store i8 0, ptr %19, align 1, !tbaa !12
  %20 = add nsw i32 %18, 1
  %exitcond2 = icmp eq i32 %20, 0
  br i1 %exitcond2, label %21, label %17, !llvm.loop !16

21:
  %22 = add nuw nsw i32 %16, 1
  %23 = getelementptr i8, ptr %15, i32 54
  %exitcond3 = icmp eq i32 %22, 16
  br i1 %exitcond3, label %24, label %14, !llvm.loop !17

24:
  %25 = getelementptr i8, ptr @g, i32 54
  br label %26

26:
  %27 = phi ptr [ %25, %24 ], [ %40, %38 ]
  %28 = phi i32 [ %39, %38 ], [ 0, %24 ]
  %29 = phi i32 [ %36, %38 ], [ 0, %24 ]
  br label %30

30:
  %31 = phi i32 [ %37, %30 ], [ -54, %26 ]
  %32 = phi i32 [ %29, %26 ], [ %36, %30 ]
  %33 = getelementptr i8, ptr %27, i32 %31
  %34 = load i8, ptr %33, align 1, !tbaa !12
  %35 = sext i8 %34 to i32
  %36 = add nsw i32 %32, %35
  %37 = add nsw i32 %31, 1
  %exitcond4 = icmp eq i32 %37, 0
  br i1 %exitcond4, label %38, label %30, !llvm.loop !18

38:
  %39 = add nuw nsw i32 %28, 1
  %40 = getelementptr i8, ptr %27, i32 54
  %exitcond5 = icmp eq i32 %39, 16
  br i1 %exitcond5, label %41, label %26, !llvm.loop !19

41:
  ret i32 %36
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %1 = tail call i32 @bench() nobuiltin "no-builtins"
  tail call void @report(i32 noundef %1) nobuiltin "no-builtins"
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
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !6, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"short", !6, i64 0}
!12 = !{!6, !6, i64 0}
!13 = distinct !{!13, !14}
!14 = !{!"llvm.loop.mustprogress"}
!15 = distinct !{!15, !14}
!16 = distinct !{!16, !14}
!17 = distinct !{!17, !14}
!18 = distinct !{!18, !14}
!19 = distinct !{!19, !14}
