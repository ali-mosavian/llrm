target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

@_peg = internal unnamed_addr global [3 x [16 x i32]] zeroinitializer, align 4
@_top = internal unnamed_addr global [3 x i32] zeroinitializer, align 4
@_moves = internal unnamed_addr global i32 0, align 4

define cc 1002 i32 @bench_x_hanoi2_(i32 inreg noundef %0) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" memory(readwrite, inaccessiblemem: none) nofree nosync {
  %promoted = load i32, ptr @_top, align 1
  %2 = icmp sgt i32 %0, 0
  br i1 %2, label %3, label %10

3:
  %4 = phi i32 [ %9, %3 ], [ 0, %1 ]
  %5 = phi i32 [ %7, %3 ], [ %promoted, %1 ]
  %6 = sub nuw nsw i32 %0, %4
  %7 = add nsw i32 %5, 1
  %8 = getelementptr inbounds [4 x i8], ptr @_peg, i32 %5
  store i32 %6, ptr %8, align 4, !tbaa !8
  %9 = add nuw nsw i32 %4, 1
  %exitcond = icmp eq i32 %9, %0
  br i1 %exitcond, label %10, label %3

10:
  %11 = phi i32 [ %promoted, %1 ], [ %7, %3 ]
  store i32 %11, ptr @_top, align 1
  tail call cc 1002 void @mv_(i32 inreg noundef %0, i32 inreg noundef 0, i32 inreg noundef 2, i32 inreg noundef 1)
  %12 = load i32, ptr @_moves, align 4, !tbaa !9
  %13 = mul nsw i32 %12, 100
  %14 = load i32, ptr getelementptr inbounds nuw (i8, ptr @_top, i32 8), align 4, !tbaa !8
  %15 = add nsw i32 %13, %14
  ret i32 %15
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal cc 1002 void @mv_(i32 inreg noundef %0, i32 inreg noundef range(i32 0, 3) %1, i32 inreg noundef range(i32 0, 3) %2, i32 inreg noundef range(i32 0, 3) %3) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" nearcode memory(readwrite, argmem: none, inaccessiblemem: none) nofree nosync {
  %5 = icmp eq i32 %0, 0
  br i1 %5, label %26, label %6

6:
  %7 = getelementptr inbounds nuw [64 x i8], ptr @_peg, i32 %2
  %8 = getelementptr inbounds nuw [4 x i8], ptr @_top, i32 %2
  br label %9

9:
  %10 = phi i32 [ %3, %6 ], [ %11, %9 ]
  %11 = phi i32 [ %1, %6 ], [ %10, %9 ]
  %12 = phi i32 [ %0, %6 ], [ %13, %9 ]
  %13 = add nsw i32 %12, -1
  tail call cc 1002 void @mv_(i32 inreg noundef %13, i32 inreg noundef %11, i32 inreg noundef %10, i32 inreg noundef %2)
  %14 = getelementptr inbounds nuw [64 x i8], ptr @_peg, i32 %11
  %15 = getelementptr inbounds nuw [4 x i8], ptr @_top, i32 %11
  %16 = load i32, ptr %15, align 4, !tbaa !8
  %17 = add nsw i32 %16, -1
  store i32 %17, ptr %15, align 4, !tbaa !8
  %18 = getelementptr inbounds [4 x i8], ptr %14, i32 %17
  %19 = load i32, ptr %18, align 4, !tbaa !8
  %20 = load i32, ptr %8, align 4, !tbaa !8
  %21 = add nsw i32 %20, 1
  store i32 %21, ptr %8, align 4, !tbaa !8
  %22 = getelementptr inbounds [4 x i8], ptr %7, i32 %20
  store i32 %19, ptr %22, align 4, !tbaa !8
  %23 = load i32, ptr @_moves, align 4, !tbaa !9
  %24 = add nsw i32 %23, 1
  store i32 %24, ptr @_moves, align 4, !tbaa !9
  %25 = icmp eq i32 %13, 0
  br i1 %25, label %26, label %9

26:
  ret void
}

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define cc 1002 i32 @main_() nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" {
  %promoted = load i32, ptr @_top, align 1
  %1 = getelementptr [4 x i8], ptr @_peg, i32 %promoted
  store i32 12, ptr %1, align 4, !tbaa !8
  %2 = getelementptr i8, ptr %1, i32 4
  store i32 11, ptr %2, align 4, !tbaa !8
  %3 = getelementptr i8, ptr %1, i32 8
  store i32 10, ptr %3, align 4, !tbaa !8
  %4 = getelementptr i8, ptr %1, i32 12
  store i32 9, ptr %4, align 4, !tbaa !8
  %5 = getelementptr i8, ptr %1, i32 16
  store i32 8, ptr %5, align 4, !tbaa !8
  %6 = getelementptr i8, ptr %1, i32 20
  store i32 7, ptr %6, align 4, !tbaa !8
  %7 = getelementptr i8, ptr %1, i32 24
  store i32 6, ptr %7, align 4, !tbaa !8
  %8 = getelementptr i8, ptr %1, i32 28
  store i32 5, ptr %8, align 4, !tbaa !8
  %9 = getelementptr i8, ptr %1, i32 32
  store i32 4, ptr %9, align 4, !tbaa !8
  %10 = getelementptr i8, ptr %1, i32 36
  store i32 3, ptr %10, align 4, !tbaa !8
  %11 = getelementptr i8, ptr %1, i32 40
  store i32 2, ptr %11, align 4, !tbaa !8
  %12 = add nsw i32 %promoted, 12
  %13 = getelementptr i8, ptr %1, i32 44
  store i32 1, ptr %13, align 4, !tbaa !8
  store i32 %12, ptr @_top, align 1
  tail call cc 1002 void @mv_(i32 inreg noundef 12, i32 inreg noundef 0, i32 inreg noundef 2, i32 inreg noundef 1)
  %14 = load i32, ptr @_moves, align 4, !tbaa !9
  %15 = mul nsw i32 %14, 100
  %16 = load i32, ptr getelementptr inbounds nuw (i8, ptr @_top, i32 8), align 4, !tbaa !8
  %17 = add nsw i32 %15, %16
  tail call cc 1002 void @report_(i32 inreg noundef %17)
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
!8 = !{!5, !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"long", !6, i64 0}
