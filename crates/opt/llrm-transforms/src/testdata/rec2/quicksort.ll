target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

define cc 1002 i32 @bench_quicksort_(i16 inreg noundef zeroext %0) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" nofree nosync memory(none) {
  %2 = alloca [1024 x i32], align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  br label %3

3:
  %4 = phi i16 [ %0, %1 ], [ %7, %3 ]
  %5 = phi i32 [ 0, %1 ], [ %11, %3 ]
  %6 = mul i16 %4, 25173
  %7 = add i16 %6, 13849
  %8 = and i16 %7, 32767
  %9 = zext nneg i16 %8 to i32
  %10 = getelementptr inbounds nuw [4 x i8], ptr %2, i32 %5
  store i32 %9, ptr %10, align 4, !tbaa !12
  %11 = add nuw nsw i32 %5, 1
  %exitcond = icmp eq i32 %11, 1024
  br i1 %exitcond, label %12, label %3

12:
  call cc 1002 void @sort_(ptr inreg noundef %2, i32 inreg noundef 0, i32 inreg noundef 1023)
  br label %15

13:
  %14 = add nuw nsw i32 %16, 1
  %exitcond1 = icmp eq i32 %14, 1024
  br i1 %exitcond1, label %22, label %15

15:
  %16 = phi i32 [ 1, %12 ], [ %14, %13 ]
  %17 = getelementptr [4 x i8], ptr %2, i32 %16
  %18 = getelementptr i8, ptr %17, i32 -4
  %19 = load i32, ptr %18, align 4, !tbaa !12
  %20 = load i32, ptr %17, align 4, !tbaa !12
  %21 = icmp sgt i32 %19, %20
  br i1 %21, label %32, label %13

22:
  %23 = phi i32 [ %30, %22 ], [ 0, %13 ]
  %24 = phi i32 [ %31, %22 ], [ 0, %13 ]
  %25 = getelementptr inbounds nuw [4 x i8], ptr %2, i32 %24
  %26 = load i32, ptr %25, align 4, !tbaa !12
  %27 = and i32 %24, 15
  %28 = add nuw nsw i32 %27, 1
  %29 = mul nsw i32 %26, %28
  %30 = add nsw i32 %29, %23
  %31 = add nuw nsw i32 %24, 1
  %exitcond2 = icmp eq i32 %31, 1024
  br i1 %exitcond2, label %32, label %22

32:
  %33 = phi i32 [ %30, %22 ], [ -1, %15 ]
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  ret i32 %33
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal cc 1002 void @sort_(ptr inreg noundef nonnull nocapture nofree %0, i32 inreg noundef range(i32 -2147483647, -2147483648) %1, i32 inreg noundef range(i32 -2147483648, 2147483647) %2) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" nearcode nofree nosync memory(argmem: readwrite) {
  %4 = icmp slt i32 %1, %2
  br i1 %4, label %5, label %31

5:
  %6 = getelementptr inbounds [4 x i8], ptr %0, i32 %2
  br label %7

7:
  %8 = phi i32 [ %1, %5 ], [ %29, %24 ]
  %9 = load i32, ptr %6, align 4, !tbaa !12
  br label %10

10:
  %11 = phi i32 [ %8, %7 ], [ %22, %20 ]
  %12 = phi i32 [ %8, %7 ], [ %21, %20 ]
  %13 = getelementptr inbounds [4 x i8], ptr %0, i32 %11
  %14 = load i32, ptr %13, align 4, !tbaa !12
  %15 = icmp slt i32 %14, %9
  br i1 %15, label %16, label %20

16:
  %17 = getelementptr inbounds [4 x i8], ptr %0, i32 %12
  %18 = load i32, ptr %17, align 4, !tbaa !12
  store i32 %14, ptr %17, align 4, !tbaa !12
  store i32 %18, ptr %13, align 4, !tbaa !12
  %19 = add nsw i32 %12, 1
  br label %20

20:
  %21 = phi i32 [ %19, %16 ], [ %12, %10 ]
  %22 = add nsw i32 %11, 1
  %23 = icmp slt i32 %22, %2
  br i1 %23, label %10, label %24

24:
  %25 = getelementptr inbounds [4 x i8], ptr %0, i32 %21
  %26 = load i32, ptr %25, align 4, !tbaa !12
  %27 = load i32, ptr %6, align 4, !tbaa !12
  store i32 %27, ptr %25, align 4, !tbaa !12
  store i32 %26, ptr %6, align 4, !tbaa !12
  %28 = add nsw i32 %21, -1
  tail call cc 1002 void @sort_(ptr inreg noundef %0, i32 inreg noundef %8, i32 inreg noundef %28)
  %29 = add nsw i32 %21, 1
  %30 = icmp slt i32 %29, %2
  br i1 %30, label %7, label %31

31:
  ret void
}

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define cc 1002 i32 @main_() nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" {
  %1 = alloca [1024 x i32], align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  br label %2

2:
  %3 = phi i16 [ 1, %0 ], [ %6, %2 ]
  %4 = phi i32 [ 0, %0 ], [ %10, %2 ]
  %5 = mul i16 %3, 25173
  %6 = add i16 %5, 13849
  %7 = and i16 %6, 32767
  %8 = zext nneg i16 %7 to i32
  %9 = getelementptr inbounds nuw [4 x i8], ptr %1, i32 %4
  store i32 %8, ptr %9, align 4, !tbaa !12
  %10 = add nuw nsw i32 %4, 1
  %exitcond = icmp eq i32 %10, 1024
  br i1 %exitcond, label %11, label %2

11:
  call cc 1002 void @sort_(ptr inreg noundef %1, i32 inreg noundef 0, i32 inreg noundef 1023)
  br label %14

12:
  %13 = add nuw nsw i32 %15, 1
  %exitcond1 = icmp eq i32 %13, 1024
  br i1 %exitcond1, label %21, label %14

14:
  %15 = phi i32 [ 1, %11 ], [ %13, %12 ]
  %16 = getelementptr [4 x i8], ptr %1, i32 %15
  %17 = getelementptr i8, ptr %16, i32 -4
  %18 = load i32, ptr %17, align 4, !tbaa !12
  %19 = load i32, ptr %16, align 4, !tbaa !12
  %20 = icmp sgt i32 %18, %19
  br i1 %20, label %31, label %12

21:
  %22 = phi i32 [ %29, %21 ], [ 0, %12 ]
  %23 = phi i32 [ %30, %21 ], [ 0, %12 ]
  %24 = getelementptr inbounds nuw [4 x i8], ptr %1, i32 %23
  %25 = load i32, ptr %24, align 4, !tbaa !12
  %26 = and i32 %23, 15
  %27 = add nuw nsw i32 %26, 1
  %28 = mul nsw i32 %27, %25
  %29 = add nsw i32 %28, %22
  %30 = add nuw nsw i32 %23, 1
  %exitcond2 = icmp eq i32 %30, 1024
  br i1 %exitcond2, label %31, label %21

31:
  %32 = phi i32 [ %29, %21 ], [ -1, %14 ]
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  tail call cc 1002 void @report_(i32 inreg noundef %32)
  ret i32 0
}

declare cc 1002 void @report_(i32 inreg noundef) "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87"

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
!10 = !{!11, !11, i64 0}
!11 = !{!"long", !6, i64 0}
!12 = !{!5, !5, i64 0}
!13 = !{!14, !14, i64 0}
!14 = !{!"p1 int", !15, i64 0}
!15 = !{!"any pointer", !6, i64 0}
