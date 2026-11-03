target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

define i32 @place(ptr noundef nonnull nocapture nofree %0, i32 noundef %1, i32 noundef %2) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" nofree nosync memory(argmem: readwrite) {
  %4 = icmp eq i32 %1, %2
  br i1 %4, label %35, label %5

5:
  %6 = icmp sgt i32 %2, 0
  br i1 %6, label %7, label %35

7:
  %8 = getelementptr inbounds [4 x i8], ptr %0, i32 %1
  %9 = add nsw i32 %1, 1
  %10 = sub i32 0, %1
  br label %11

11:
  %12 = phi i32 [ 0, %7 ], [ %33, %32 ]
  %13 = phi i32 [ 0, %7 ], [ %34, %32 ]
  %14 = icmp sgt i32 %1, 0
  br i1 %14, label %15, label %29

15:
  br label %20

16:
  %17 = add i32 %23, 1
  %18 = add i32 %22, -1
  %19 = getelementptr i8, ptr %21, i32 4
  %exitcond = icmp eq i32 %18, 0
  br i1 %exitcond, label %29, label %20, !llvm.loop !13

20:
  %21 = phi ptr [ %0, %15 ], [ %19, %16 ]
  %22 = phi i32 [ %1, %15 ], [ %18, %16 ]
  %23 = phi i32 [ %10, %15 ], [ %17, %16 ]
  %24 = load i32, ptr %21, align 4, !tbaa !6
  %25 = sub nsw i32 %24, %13
  %26 = icmp eq i32 %24, %13
  %27 = icmp eq i32 %22, %25
  %or.cond = select i1 %26, i1 true, i1 %27
  %28 = icmp eq i32 %23, %25
  %or.cond1 = select i1 %or.cond, i1 true, i1 %28
  br i1 %or.cond1, label %32, label %16

29:
  store i32 %13, ptr %8, align 4, !tbaa !6
  %30 = tail call i32 @place(ptr noundef %0, i32 noundef %9, i32 noundef %2) nobuiltin "no-builtins"
  %31 = add nsw i32 %30, %12
  br label %32

32:
  %33 = phi i32 [ %31, %29 ], [ %12, %20 ]
  %34 = add nuw nsw i32 %13, 1
  %exitcond1 = icmp eq i32 %34, %2
  br i1 %exitcond1, label %35, label %11, !llvm.loop !10

35:
  %36 = phi i32 [ 1, %3 ], [ 0, %5 ], [ %33, %32 ]
  ret i32 %36
}

!6 = !{!7, !7, i64 0}
!7 = !{!"int", !8, i64 0}
!8 = !{!"omnipotent char", !9, i64 0}
!9 = !{!"Simple C/C++ TBAA"}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!13 = distinct !{!13, !11}
