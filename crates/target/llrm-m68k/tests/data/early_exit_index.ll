target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

@main.at = internal unnamed_addr constant [14 x i32] [i32 0, i32 1, i32 7, i32 8, i32 15, i32 16, i32 17, i32 31, i32 32, i32 599, i32 1183, i32 1184, i32 1199, i32 -1], align 2
@b = internal unnamed_addr global [1200 x i8] zeroinitializer, align 1
@a = internal unnamed_addr global [1200 x i8] zeroinitializer, align 1

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  br label %2

1:
  tail call void @report(i32 noundef %80) nobuiltin "no-builtins"
  ret i32 0

2:
  %3 = phi i32 [ 0, %0 ], [ %81, %76 ]
  %4 = phi i32 [ 0, %0 ], [ %80, %76 ]
  %5 = getelementptr inbounds nuw [4 x i8], ptr @main.at, i32 %3
  %6 = load i32, ptr %5, align 2, !tbaa !8
  br label %9

7:
  %8 = icmp eq i32 %3, 13
  br i1 %8, label %22, label %18

9:
  %10 = phi i32 [ %17, %9 ], [ -1200, %2 ]
  %11 = phi i8 [ %16, %9 ], [ 0, %2 ]
  %12 = phi ptr [ %15, %9 ], [ @b, %2 ]
  %13 = phi ptr [ %14, %9 ], [ @a, %2 ]
  store i8 %11, ptr %12, align 1, !tbaa !9
  store i8 %11, ptr %13, align 1, !tbaa !9
  %14 = getelementptr i8, ptr %13, i32 1
  %15 = getelementptr i8, ptr %12, i32 1
  %16 = add i8 %11, 7
  %17 = add nsw i32 %10, 1
  %exitcond3 = icmp eq i32 %17, 0
  br i1 %exitcond3, label %7, label %9, !llvm.loop !10

18:
  %19 = getelementptr inbounds nuw i8, ptr @a, i32 %6
  %20 = load i8, ptr %19, align 1, !tbaa !9
  %21 = xor i8 %20, -128
  store i8 %21, ptr %19, align 1, !tbaa !9
  br label %22

22:
  %23 = phi i32 [ 65535, %7 ], [ %6, %18 ]
  br label %24

24:
  %25 = phi ptr [ %32, %33 ], [ @a, %22 ]
  %26 = phi ptr [ %31, %33 ], [ @b, %22 ]
  %27 = phi i32 [ 0, %22 ], [ %34, %33 ]
  %28 = load i8, ptr %25, align 1, !tbaa !9
  %29 = load i8, ptr %26, align 1, !tbaa !9
  %30 = icmp eq i8 %28, %29
  %31 = getelementptr i8, ptr %26, i32 1
  %32 = getelementptr i8, ptr %25, i32 1
  br i1 %30, label %33, label %35

33:
  %34 = add nuw nsw i32 %27, 1
  %exitcond2 = icmp eq i32 %34, 1200
  br i1 %exitcond2, label %37, label %24, !llvm.loop !13

35:
  %36 = phi i32 [ %27, %24 ]
  br label %38

37:
  br label %38

38:
  %39 = phi i32 [ %36, %35 ], [ -1, %37 ]
  %40 = icmp ne i32 %39, %6
  %41 = zext i1 %40 to i32
  %42 = add nsw i32 %4, %41
  br label %43

43:
  %44 = phi ptr [ %51, %52 ], [ @a, %38 ]
  %45 = phi ptr [ %50, %52 ], [ @b, %38 ]
  %46 = phi i32 [ 0, %38 ], [ %53, %52 ]
  %47 = load i8, ptr %44, align 1, !tbaa !9
  %48 = load i8, ptr %45, align 1, !tbaa !9
  %49 = icmp eq i8 %47, %48
  %50 = getelementptr i8, ptr %45, i32 1
  %51 = getelementptr i8, ptr %44, i32 1
  br i1 %49, label %52, label %54

52:
  %53 = add nuw nsw i32 %46, 1
  %exitcond1 = icmp eq i32 %53, 1200
  br i1 %exitcond1, label %56, label %43, !llvm.loop !16

54:
  %55 = phi i32 [ %46, %43 ]
  br label %57

56:
  br label %57

57:
  %58 = phi i32 [ %55, %54 ], [ -1, %56 ]
  %59 = icmp ne i32 %58, %6
  %60 = zext i1 %59 to i32
  %61 = add nsw i32 %42, %60
  br label %62

62:
  %63 = phi ptr [ %70, %71 ], [ @a, %57 ]
  %64 = phi ptr [ %69, %71 ], [ @b, %57 ]
  %65 = phi i32 [ 0, %57 ], [ %72, %71 ]
  %66 = load i8, ptr %63, align 1, !tbaa !9
  %67 = load i8, ptr %64, align 1, !tbaa !9
  %68 = icmp eq i8 %66, %67
  %69 = getelementptr i8, ptr %64, i32 1
  %70 = getelementptr i8, ptr %63, i32 1
  br i1 %68, label %71, label %74

71:
  %72 = add nuw nsw i32 %65, 1
  %exitcond = icmp eq i32 %72, 1200
  br i1 %exitcond, label %73, label %62, !llvm.loop !17

73:
  br label %76

74:
  %75 = phi i32 [ %65, %62 ]
  br label %76

76:
  %77 = phi i32 [ 65535, %73 ], [ %75, %74 ]
  %78 = icmp ne i32 %23, %77
  %79 = zext i1 %78 to i32
  %80 = add nsw i32 %61, %79
  %81 = add nuw nsw i32 %3, 1
  %exitcond4 = icmp eq i32 %81, 14
  br i1 %exitcond4, label %1, label %2, !llvm.loop !12
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

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
!9 = !{!6, !6, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
!13 = distinct !{!13, !11}
!14 = !{!15, !15, i64 0}
!15 = !{!"long", !6, i64 0}
!16 = distinct !{!16, !11}
!17 = distinct !{!17, !11}
