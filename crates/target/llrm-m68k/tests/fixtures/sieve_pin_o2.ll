source_filename = "bench/sieve/sieve.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @bench_sieve(i16 noundef zeroext %0) local_unnamed_addr #0 {
  %2 = alloca [1024 x i8], align 1
  call void @llvm.lifetime.start.p0(ptr nonnull %2) #5
  %3 = icmp eq i16 %0, 0
  br i1 %3, label %41, label %4

4:                                                ; preds = %1
  %5 = zext i16 %0 to i32
  call void @llvm.memset.p0.i32(ptr nonnull align 1 %2, i8 0, i32 %5, i1 false), !tbaa !6
  %6 = icmp ugt i16 %0, 2
  br i1 %6, label %7, label %41

7:                                                ; preds = %4
  %8 = zext i16 %0 to i32
  br label %9

9:                                                ; preds = %7, %32
  %10 = phi i32 [ 2, %7 ], [ %35, %32 ]
  %11 = phi i32 [ 0, %7 ], [ %34, %32 ]
  %12 = phi i16 [ 0, %7 ], [ %33, %32 ]
  %13 = getelementptr inbounds nuw i8, ptr %2, i32 %10
  %14 = load i8, ptr %13, align 1, !tbaa !6
  %15 = icmp eq i8 %14, 0
  br i1 %15, label %16, label %32

16:                                               ; preds = %9
  %17 = add i16 %12, 1
  %18 = add i32 %11, %10
  %19 = icmp samesign ult i32 %10, 32
  %20 = mul nuw nsw i32 %10, %10
  %21 = icmp samesign ult i32 %20, %8
  %22 = select i1 %19, i1 %21, i1 false
  br i1 %22, label %23, label %32

23:                                               ; preds = %16
  %24 = trunc nuw i32 %20 to i16
  %25 = trunc nuw nsw i32 %10 to i16
  br label %26

26:                                               ; preds = %23, %26
  %27 = phi i16 [ %30, %26 ], [ %24, %23 ]
  %28 = zext i16 %27 to i32
  %29 = getelementptr inbounds nuw i8, ptr %2, i32 %28
  store i8 1, ptr %29, align 1, !tbaa !6
  %30 = add i16 %27, %25
  %31 = icmp ult i16 %30, %0
  br i1 %31, label %26, label %32, !llvm.loop !7

32:                                               ; preds = %26, %16, %9
  %33 = phi i16 [ %12, %9 ], [ %17, %16 ], [ %17, %26 ]
  %34 = phi i32 [ %11, %9 ], [ %18, %16 ], [ %18, %26 ]
  %35 = add nuw nsw i32 %10, 1
  %36 = icmp eq i32 %35, %8
  br i1 %36, label %37, label %9, !llvm.loop !9

37:                                               ; preds = %32
  %38 = zext i16 %33 to i32
  %39 = shl nuw i32 %38, 16
  %40 = xor i32 %39, %34
  br label %41

41:                                               ; preds = %1, %37, %4
  %42 = phi i32 [ 0, %4 ], [ %40, %37 ], [ 0, %1 ]
  call void @llvm.lifetime.end.p0(ptr nonnull %2) #5
  ret i32 %42
}

declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

define noundef i32 @main() local_unnamed_addr #2 {
  %1 = alloca [1024 x i8], align 1
  call void @llvm.lifetime.start.p0(ptr nonnull %1) #5
  call void @llvm.memset.p0.i32(ptr noundef nonnull align 1 dereferenceable(1024) %1, i8 0, i32 1024, i1 false), !tbaa !6
  br label %2

2:                                                ; preds = %22, %0
  %3 = phi i32 [ %27, %22 ], [ 5, %0 ]
  %4 = phi i16 [ %29, %22 ], [ 4, %0 ]
  %5 = phi i32 [ %25, %22 ], [ 2, %0 ]
  %6 = phi i32 [ %24, %22 ], [ 0, %0 ]
  %7 = phi i16 [ %23, %22 ], [ 0, %0 ]
  %8 = getelementptr inbounds nuw i8, ptr %1, i32 %5
  %9 = load i8, ptr %8, align 1, !tbaa !6
  %10 = icmp eq i8 %9, 0
  br i1 %10, label %11, label %22

11:                                               ; preds = %2
  %12 = add i16 %7, 1
  %13 = add i32 %6, %5
  %14 = icmp samesign ult i32 %5, 32
  br i1 %14, label %15, label %22

15:                                               ; preds = %11
  %16 = zext i16 %4 to i32
  br label %17

17:                                               ; preds = %15, %17
  %18 = phi i32 [ %20, %17 ], [ %16, %15 ]
  %19 = getelementptr inbounds nuw i8, ptr %1, i32 %18
  store i8 1, ptr %19, align 1, !tbaa !6
  %20 = add nuw nsw i32 %18, %5
  %21 = icmp samesign ult i32 %20, 1024
  br i1 %21, label %17, label %22, !llvm.loop !7

22:                                               ; preds = %17, %11, %2
  %23 = phi i16 [ %7, %2 ], [ %12, %11 ], [ %12, %17 ]
  %24 = phi i32 [ %6, %2 ], [ %13, %11 ], [ %13, %17 ]
  %25 = add nuw nsw i32 %5, 1
  %26 = icmp eq i32 %25, 1024
  %27 = add nuw nsw i32 %3, 2
  %28 = trunc nuw nsw i32 %3 to i16
  %29 = add i16 %4, %28
  br i1 %26, label %30, label %2, !llvm.loop !9

30:                                               ; preds = %22
  %31 = zext i16 %23 to i32
  %32 = shl nuw i32 %31, 16
  %33 = xor i32 %32, %24
  call void @llvm.lifetime.end.p0(ptr nonnull %1) #5
  tail call void @report(i32 noundef %33) #5
  ret i32 0
}

declare void @report(i32 noundef) local_unnamed_addr #3

declare void @llvm.memset.p0.i32(ptr writeonly captures(none), i8, i32, i1 immarg) #4

attributes #0 = { nofree norecurse nosync nounwind memory(none) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #4 = { nocallback nofree nosync nounwind willreturn memory(argmem: write) }
attributes #5 = { nounwind }

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!4, !4, i64 0}
!7 = distinct !{!7, !8}
!8 = !{!"llvm.loop.mustprogress"}
!9 = distinct !{!9, !8}
