target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

define cc 1002 i32 @bench_bintree_(i16 inreg noundef zeroext %0) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" nofree nosync memory(none) {
  %2 = alloca [903 x i32], align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i32(ptr nonnull align 4 %2, i8 0, i32 3612, i1 false)
  br label %3

3:
  %4 = phi i16 [ %7, %25 ], [ %0, %1 ]
  %5 = phi i32 [ %26, %25 ], [ 1, %1 ]
  %6 = mul i16 %4, 25173
  %7 = add i16 %6, 13849
  %8 = and i16 %7, 4095
  %9 = zext nneg i16 %8 to i32
  %10 = mul nuw nsw i32 %5, 12
  %11 = getelementptr inbounds nuw i8, ptr %2, i32 %10
  store i32 %9, ptr %11, align 4, !tbaa !10
  %12 = icmp samesign ugt i32 %5, 1
  br i1 %12, label %13, label %25

13:
  %14 = phi i32 [ %21, %13 ], [ 1, %3 ]
  %15 = mul i32 %14, 12
  %16 = getelementptr i8, ptr %2, i32 %15
  %17 = load i32, ptr %16, align 4, !tbaa !10
  %18 = icmp sgt i32 %17, %9
  %19 = select i1 %18, i32 1, i32 2
  %20 = getelementptr [4 x i8], ptr %16, i32 %19
  %21 = load i32, ptr %20, align 4, !tbaa !10
  %22 = icmp eq i32 %21, 0
  br i1 %22, label %23, label %13

23:
  %24 = getelementptr [4 x i8], ptr %16, i32 %19
  store i32 %5, ptr %24, align 4, !tbaa !10
  br label %25

25:
  %26 = add nuw nsw i32 %5, 1
  %exitcond = icmp eq i32 %26, 301
  br i1 %exitcond, label %27, label %3

27:
  %28 = call cc 1002 i32 @walk_(ptr inreg noundef %2, i32 inreg noundef 1, i32 inreg noundef 1)
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  ret i32 %28
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal cc 1002 void @insert_(ptr inreg noundef nonnull nocapture nofree %0, i32 inreg noundef range(i32 1, 0) %1, i32 inreg noundef range(i32 2, 301) %2) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" nearcode nofree nosync memory(argmem: readwrite) norecurse {
  unreachable
}

define internal cc 1002 i32 @walk_(ptr inreg noundef nonnull nocapture nofree readonly %0, i32 inreg noundef %1, i32 inreg noundef %2) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" nearcode nofree nosync memory(argmem: read) {
  %4 = icmp eq i32 %1, 0
  br i1 %4, label %22, label %5

5:
  %6 = phi i32 [ %13, %5 ], [ %2, %3 ]
  %7 = phi i32 [ %18, %5 ], [ %1, %3 ]
  %8 = phi i32 [ %20, %5 ], [ 0, %3 ]
  %9 = mul i32 %7, 12
  %10 = getelementptr i8, ptr %0, i32 %9
  %11 = getelementptr i8, ptr %10, i32 4
  %12 = load i32, ptr %11, align 4, !tbaa !10
  %13 = add nsw i32 %6, 1
  %14 = tail call cc 1002 i32 @walk_(ptr inreg noundef %0, i32 inreg noundef %12, i32 inreg noundef %13)
  %15 = load i32, ptr %10, align 4, !tbaa !10
  %16 = mul nsw i32 %15, %6
  %17 = getelementptr i8, ptr %10, i32 8
  %18 = load i32, ptr %17, align 4, !tbaa !10
  %19 = add i32 %14, %8
  %20 = add i32 %19, %16
  %21 = icmp eq i32 %18, 0
  br i1 %21, label %22, label %5

22:
  %accumulator.tr.lcssa = phi i32 [ 0, %3 ], [ %20, %5 ]
  ret i32 %accumulator.tr.lcssa
}

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define cc 1002 i32 @main_() nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" {
  %1 = alloca [903 x i32], align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.memset.p0.i32(ptr nonnull align 4 %1, i8 0, i32 3612, i1 false)
  br label %2

2:
  %3 = phi i16 [ %6, %24 ], [ 1, %0 ]
  %4 = phi i32 [ %25, %24 ], [ 1, %0 ]
  %5 = mul i16 %3, 25173
  %6 = add i16 %5, 13849
  %7 = and i16 %6, 4095
  %8 = zext nneg i16 %7 to i32
  %9 = mul nuw nsw i32 %4, 12
  %10 = getelementptr inbounds nuw i8, ptr %1, i32 %9
  store i32 %8, ptr %10, align 4, !tbaa !10
  %11 = icmp samesign ugt i32 %4, 1
  br i1 %11, label %12, label %24

12:
  %13 = phi i32 [ %20, %12 ], [ 1, %2 ]
  %14 = mul i32 %13, 12
  %15 = getelementptr i8, ptr %1, i32 %14
  %16 = load i32, ptr %15, align 4, !tbaa !10
  %17 = icmp sgt i32 %16, %8
  %18 = select i1 %17, i32 1, i32 2
  %19 = getelementptr [4 x i8], ptr %15, i32 %18
  %20 = load i32, ptr %19, align 4, !tbaa !10
  %21 = icmp eq i32 %20, 0
  br i1 %21, label %22, label %12

22:
  %23 = getelementptr [4 x i8], ptr %15, i32 %18
  store i32 %4, ptr %23, align 4, !tbaa !10
  br label %24

24:
  %25 = add nuw nsw i32 %4, 1
  %exitcond = icmp eq i32 %25, 301
  br i1 %exitcond, label %26, label %2

26:
  %27 = call cc 1002 i32 @walk_(ptr inreg noundef %1, i32 inreg noundef 1, i32 inreg noundef 1)
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  tail call cc 1002 void @report_(i32 inreg noundef %27)
  ret i32 0
}

declare cc 1002 void @report_(i32 inreg noundef) "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87"

declare void @llvm.memset.p0.i32(ptr nocapture writeonly, i8, i32, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.errno.tbaa = !{!3}

!0 = !{i32 1, !"NumRegisterParameters", i32 0}
!1 = !{i32 1, !"wchar_size", i32 2}
!2 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!3 = !{!4, !5, i64 0}
!4 = !{!"__libc_errno", !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
!8 = !{!9, !9, i64 0}
!9 = !{!"short", !6, i64 0}
!10 = !{!5, !5, i64 0}
!11 = !{!12, !12, i64 0}
!12 = !{!"p1 int", !13, i64 0}
!13 = !{!"any pointer", !6, i64 0}
