target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

@__main_void = hidden alias i32 (), ptr @main

define hidden i32 @bench_sieve(i16 noundef zeroext %0) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" memory(none) nofree nosync norecurse {
  %2 = alloca [1024 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  %3 = icmp eq i16 %0, 0
  br i1 %3, label %6, label %4

4:
  %5 = zext i16 %0 to i32
  br label %10

6:
  %7 = icmp ugt i16 %0, 2
  br i1 %7, label %8, label %45

8:
  %9 = zext i16 %0 to i32
  %wide.trip.count = zext i16 %0 to i32
  br label %15

10:
  %11 = phi ptr [ %14, %10 ], [ %2, %4 ]
  %12 = phi i32 [ %13, %10 ], [ %5, %4 ]
  store i8 0, ptr %11, align 1, !tbaa !10
  %13 = add nsw i32 %12, -1
  %14 = getelementptr i8, ptr %11, i32 1
  %exitcond1 = icmp eq i32 %13, 0
  br i1 %exitcond1, label %6, label %10, !llvm.loop !11

15:
  %16 = phi i32 [ 2, %8 ], [ %40, %37 ]
  %17 = phi i32 [ %39, %37 ], [ 0, %8 ]
  %18 = phi i16 [ %38, %37 ], [ 0, %8 ]
  %19 = getelementptr inbounds nuw i8, ptr %2, i32 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = icmp eq i8 %20, 0
  br i1 %21, label %22, label %37

22:
  %23 = add i16 %18, 1
  %24 = add i32 %17, %16
  %25 = icmp samesign ult i32 %16, 32
  %26 = mul nuw nsw i32 %16, %16
  %27 = icmp samesign ult i32 %26, %9
  %or.cond = select i1 %25, i1 %27, i1 false
  br i1 %or.cond, label %28, label %37

28:
  %29 = trunc nuw nsw i32 %26 to i16
  %30 = trunc nuw nsw i32 %16 to i16
  br label %31

31:
  %32 = phi i16 [ %35, %31 ], [ %29, %28 ]
  %33 = zext i16 %32 to i32
  %34 = getelementptr inbounds nuw i8, ptr %2, i32 %33
  store i8 1, ptr %34, align 1, !tbaa !10
  %35 = add i16 %32, %30
  %36 = icmp ult i16 %35, %0
  br i1 %36, label %31, label %37, !llvm.loop !14

37:
  %38 = phi i16 [ %18, %15 ], [ %23, %22 ], [ %23, %31 ]
  %39 = phi i32 [ %17, %15 ], [ %24, %22 ], [ %24, %31 ]
  %40 = add nuw nsw i32 %16, 1
  %exitcond = icmp eq i32 %40, %wide.trip.count
  br i1 %exitcond, label %41, label %15, !llvm.loop !15

41:
  %42 = zext i16 %38 to i32
  %43 = shl nuw i32 %42, 16
  %44 = xor i32 %43, %39
  br label %45

45:
  %46 = phi i32 [ %44, %41 ], [ 0, %6 ]
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  ret i32 %46
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define hidden i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" {
  %1 = alloca [1024 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  br label %2

2:
  %3 = phi i32 [ %6, %2 ], [ -1024, %0 ]
  %4 = getelementptr i8, ptr %1, i32 %3
  %5 = getelementptr i8, ptr %4, i32 1024
  store i8 0, ptr %5, align 1, !tbaa !10
  %6 = add nsw i32 %3, 1
  %exitcond1 = icmp eq i32 %6, 0
  br i1 %exitcond1, label %7, label %2, !llvm.loop !11

7:
  br label %8

8:
  %9 = phi i32 [ %33, %29 ], [ 5, %7 ]
  %10 = phi i16 [ %35, %29 ], [ 4, %7 ]
  %11 = phi i32 [ %32, %29 ], [ 2, %7 ]
  %12 = phi i32 [ %31, %29 ], [ 0, %7 ]
  %13 = phi i16 [ %30, %29 ], [ 0, %7 ]
  %14 = getelementptr inbounds nuw i8, ptr %1, i32 %11
  %15 = load i8, ptr %14, align 1, !tbaa !10
  %16 = icmp eq i8 %15, 0
  br i1 %16, label %17, label %29

17:
  %18 = add i16 %13, 1
  %19 = add i32 %12, %11
  %20 = icmp samesign ult i32 %11, 32
  %21 = icmp samesign ult i32 %11, 32
  %or.cond = select i1 %20, i1 %21, i1 false
  br i1 %or.cond, label %22, label %29

22:
  %23 = zext i16 %10 to i32
  br label %24

24:
  %25 = phi i32 [ %27, %24 ], [ %23, %22 ]
  %26 = getelementptr i8, ptr %1, i32 %25
  store i8 1, ptr %26, align 1, !tbaa !10
  %27 = add nuw nsw i32 %25, %11
  %28 = icmp samesign ult i32 %27, 1024
  br i1 %28, label %24, label %29, !llvm.loop !14

29:
  %30 = phi i16 [ %13, %8 ], [ %18, %17 ], [ %18, %24 ]
  %31 = phi i32 [ %12, %8 ], [ %19, %17 ], [ %19, %24 ]
  %32 = add nuw nsw i32 %11, 1
  %exitcond = icmp eq i32 %32, 1024
  %33 = add nuw nsw i32 %9, 2
  %34 = trunc nuw nsw i32 %9 to i16
  %35 = add i16 %10, %34
  br i1 %exitcond, label %36, label %8, !llvm.loop !15

36:
  %37 = zext i16 %30 to i32
  %38 = shl nuw i32 %37, 16
  %39 = xor i32 %38, %31
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  tail call void @report(i32 noundef %39) nobuiltin "no-builtins"
  ret i32 0
}

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext"

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!7, !7, i64 0}
!7 = !{!"short", !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !4, i64 0}
!10 = !{!4, !4, i64 0}
!11 = distinct !{!11, !12, !13}
!12 = !{!"llvm.loop.mustprogress"}
!13 = !{!"llvm.loop.unroll.disable"}
!14 = distinct !{!14, !12, !13}
!15 = distinct !{!15, !12, !13}

