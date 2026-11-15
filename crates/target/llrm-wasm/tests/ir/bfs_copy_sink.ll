target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

@queue = internal unnamed_addr global [80 x i8] zeroinitializer, align 16
@nxt = internal unnamed_addr global [2560 x i8] zeroinitializer, align 16
@fail = internal unnamed_addr global [80 x i8] zeroinitializer, align 16
@hit = internal unnamed_addr global [80 x i8] zeroinitializer, align 16

define hidden i32 @bfs(i32 noundef %0, i32 noundef %1, i32 noundef %2) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" memory(readwrite, argmem: none, inaccessiblemem: none) willreturn nofree nosync norecurse {
  %4 = icmp slt i32 %0, %1
  br i1 %4, label %5, label %51

5:
  br label %9

6:
  %7 = phi i32 [ %11, %9 ], [ %49, %48 ]
  %8 = icmp slt i32 %13, %7
  br i1 %8, label %9, label %51

9:
  %10 = phi i32 [ %0, %5 ], [ %13, %6 ]
  %11 = phi i32 [ %1, %5 ], [ %7, %6 ]
  %12 = icmp sgt i32 %2, 0
  %13 = add nsw i32 %10, 1
  br i1 %12, label %14, label %6, !llvm.loop !11

14:
  %15 = getelementptr inbounds i8, ptr @queue, i32 %10
  %16 = load i8, ptr %15, align 1, !tbaa !7
  %17 = zext i8 %16 to i32
  %18 = shl nuw nsw i32 %17, 5
  %19 = getelementptr inbounds nuw i8, ptr @nxt, i32 %18
  br label %20

20:
  %21 = phi i32 [ 0, %14 ], [ %50, %48 ]
  %22 = phi i32 [ %11, %14 ], [ %49, %48 ]
  %23 = getelementptr i8, ptr %19, i32 %21
  %24 = load i8, ptr %23, align 1, !tbaa !7
  %25 = zext i8 %24 to i32
  %26 = getelementptr inbounds i8, ptr @fail, i32 %17
  %27 = load i8, ptr %26, align 1, !tbaa !7
  %28 = zext i8 %27 to i32
  %29 = shl nuw nsw i32 %28, 5
  %30 = add i32 %21, %29
  %31 = getelementptr i8, ptr @nxt, i32 %30
  %32 = load i8, ptr %31, align 1, !tbaa !7
  %33 = zext i8 %32 to i32
  %34 = icmp eq i8 %24, 0
  br i1 %34, label %47, label %35

35:
  %36 = getelementptr inbounds nuw i8, ptr @fail, i32 %25
  store i8 %32, ptr %36, align 1, !tbaa !7
  %37 = getelementptr inbounds nuw i8, ptr @hit, i32 %25
  %38 = load i8, ptr %37, align 1, !tbaa !7
  %39 = icmp eq i8 %38, 0
  br i1 %39, label %40, label %44

40:
  %41 = getelementptr inbounds nuw i8, ptr @hit, i32 %33
  %42 = load i8, ptr %41, align 1, !tbaa !7
  %43 = getelementptr inbounds i8, ptr @hit, i32 %25
  store i8 %42, ptr %43, align 1, !tbaa !7
  br label %44

44:
  %45 = add nsw i32 %22, 1
  %46 = getelementptr inbounds i8, ptr @queue, i32 %22
  store i8 %24, ptr %46, align 1, !tbaa !7
  br label %48

47:
  store i8 %32, ptr %23, align 1, !tbaa !7
  br label %48

48:
  %49 = phi i32 [ %45, %44 ], [ %22, %47 ]
  %50 = add nuw nsw i32 %21, 1
  %exitcond = icmp eq i32 %2, %50
  br i1 %exitcond, label %6, label %20, !llvm.loop !8

51:
  %52 = phi i32 [ %1, %3 ], [ %7, %6 ]
  ret i32 %52
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!3, !3, i64 0}
!7 = !{!4, !4, i64 0}
!8 = distinct !{!8, !9, !10}
!9 = !{!"llvm.loop.mustprogress"}
!10 = !{!"llvm.loop.unroll.disable"}
!11 = distinct !{!11, !9, !10}
