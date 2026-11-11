target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

@_a = internal unnamed_addr global [256 x i32] zeroinitializer, align 4
@_b = internal unnamed_addr global [256 x i32] zeroinitializer, align 4

define cc 1002 i32 @bench_x_mergesort_(i32 inreg noundef %0) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" memory(readwrite, inaccessiblemem: none) nofree nosync {
  %2 = icmp sgt i32 %0, 0
  br i1 %2, label %3, label %11

3:
  %4 = phi i32 [ %10, %3 ], [ 0, %1 ]
  %5 = phi i32 [ %7, %3 ], [ 99, %1 ]
  %6 = mul i32 %5, 69069
  %7 = add i32 %6, 1
  %8 = lshr i32 %7, 18
  %9 = getelementptr inbounds nuw [4 x i8], ptr @_a, i32 %4
  store i32 %8, ptr %9, align 4, !tbaa !8
  %10 = add nuw nsw i32 %4, 1
  %exitcond = icmp eq i32 %10, %0
  br i1 %exitcond, label %12, label %3

11:
  tail call cc 1002 void @msort_(i32 inreg noundef 0, i32 inreg noundef %0)
  br label %21

12:
  tail call cc 1002 void @msort_(i32 inreg noundef 0, i32 inreg noundef %0)
  br label %13

13:
  %14 = phi i32 [ %19, %13 ], [ 0, %12 ]
  %15 = phi i32 [ %20, %13 ], [ 0, %12 ]
  %16 = mul nsw i32 %14, 11
  %17 = getelementptr inbounds nuw [4 x i8], ptr @_a, i32 %15
  %18 = load i32, ptr %17, align 4, !tbaa !8
  %19 = add nsw i32 %18, %16
  %20 = add nuw nsw i32 %15, 1
  %exitcond1 = icmp eq i32 %20, %0
  br i1 %exitcond1, label %21, label %13

21:
  %22 = phi i32 [ 0, %11 ], [ %19, %13 ]
  ret i32 %22
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal cc 1002 void @msort_(i32 inreg noundef range(i32 -1073741824, 1073741824) %0, i32 inreg noundef %1) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" nearcode memory(readwrite, argmem: none, inaccessiblemem: none) nofree nosync {
  %3 = sub nsw i32 %1, %0
  %4 = icmp slt i32 %3, 2
  br i1 %4, label %65, label %5

5:
  %6 = add nsw i32 %1, %0
  %7 = sdiv i32 %6, 2
  tail call cc 1002 void @msort_(i32 inreg noundef %0, i32 inreg noundef %7)
  tail call cc 1002 void @msort_(i32 inreg noundef %7, i32 inreg noundef %1)
  %8 = icmp slt i32 %0, %7
  %9 = icmp slt i32 %7, %1
  %10 = and i1 %8, %9
  br i1 %10, label %25, label %11

11:
  %12 = phi i32 [ %0, %5 ], [ %42, %38 ]
  %13 = phi i32 [ %7, %5 ], [ %40, %38 ]
  %14 = phi i32 [ %0, %5 ], [ %39, %38 ]
  %15 = icmp slt i32 %14, %7
  br i1 %15, label %16, label %47

16:
  %17 = shl i32 %12, 2
  %18 = getelementptr i8, ptr @_b, i32 %17
  %19 = shl i32 %14, 2
  %20 = getelementptr i8, ptr @_a, i32 %19
  %21 = sub i32 %7, %14
  %22 = shl i32 %21, 2
  call void @llvm.memcpy.p0.p0.i32(ptr align 4 %18, ptr align 4 %20, i32 %22, i1 false)
  %23 = add i32 %12, %7
  %24 = sub i32 %23, %14
  br label %47

25:
  %26 = phi i32 [ %42, %38 ], [ %0, %5 ]
  %27 = phi i32 [ %40, %38 ], [ %7, %5 ]
  %28 = phi i32 [ %39, %38 ], [ %0, %5 ]
  %29 = getelementptr inbounds [4 x i8], ptr @_a, i32 %28
  %30 = load i32, ptr %29, align 4, !tbaa !8
  %31 = getelementptr inbounds [4 x i8], ptr @_a, i32 %27
  %32 = load i32, ptr %31, align 4, !tbaa !8
  %33 = icmp sgt i32 %30, %32
  br i1 %33, label %36, label %34

34:
  %35 = add nsw i32 %28, 1
  br label %38

36:
  %37 = add nsw i32 %27, 1
  br label %38

38:
  %39 = phi i32 [ %35, %34 ], [ %28, %36 ]
  %40 = phi i32 [ %27, %34 ], [ %37, %36 ]
  %41 = phi i32 [ %30, %34 ], [ %32, %36 ]
  %42 = add nsw i32 %26, 1
  %43 = getelementptr inbounds [4 x i8], ptr @_b, i32 %26
  store i32 %41, ptr %43, align 4, !tbaa !8
  %44 = icmp slt i32 %39, %7
  %45 = icmp slt i32 %40, %1
  %46 = select i1 %44, i1 %45, i1 false
  br i1 %46, label %25, label %11

47:
  %48 = phi i32 [ %12, %11 ], [ %24, %16 ]
  %49 = icmp slt i32 %13, %1
  br i1 %49, label %50, label %57

50:
  %51 = shl i32 %48, 2
  %52 = getelementptr i8, ptr @_b, i32 %51
  %53 = shl i32 %13, 2
  %54 = getelementptr i8, ptr @_a, i32 %53
  %55 = sub i32 %1, %13
  %56 = shl i32 %55, 2
  call void @llvm.memcpy.p0.p0.i32(ptr align 4 %52, ptr align 4 %54, i32 %56, i1 false)
  br label %57

57:
  %58 = icmp slt i32 %0, %1
  br i1 %58, label %59, label %65

59:
  %60 = shl i32 %0, 2
  %61 = getelementptr i8, ptr @_a, i32 %60
  %62 = getelementptr i8, ptr @_b, i32 %60
  %63 = sub i32 %1, %0
  %64 = shl i32 %63, 2
  call void @llvm.memcpy.p0.p0.i32(ptr align 4 %61, ptr align 4 %62, i32 %64, i1 false)
  br label %65

65:
  ret void
}

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define cc 1002 i32 @main_() nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" {
  br label %1

1:
  %2 = phi i32 [ %8, %1 ], [ 0, %0 ]
  %3 = phi i32 [ %5, %1 ], [ 99, %0 ]
  %4 = mul i32 %3, 69069
  %5 = add i32 %4, 1
  %6 = lshr i32 %5, 18
  %7 = getelementptr inbounds nuw [4 x i8], ptr @_a, i32 %2
  store i32 %6, ptr %7, align 4, !tbaa !8
  %8 = add nuw nsw i32 %2, 1
  %exitcond = icmp eq i32 %8, 200
  br i1 %exitcond, label %9, label %1

9:
  tail call cc 1002 void @msort_(i32 inreg noundef 0, i32 inreg noundef 200)
  br label %10

10:
  %11 = phi i32 [ %16, %10 ], [ 0, %9 ]
  %12 = phi i32 [ %17, %10 ], [ 0, %9 ]
  %13 = mul nsw i32 %11, 11
  %14 = getelementptr inbounds nuw [4 x i8], ptr @_a, i32 %12
  %15 = load i32, ptr %14, align 4, !tbaa !8
  %16 = add nsw i32 %15, %13
  %17 = add nuw nsw i32 %12, 1
  %exitcond1 = icmp eq i32 %17, 200
  br i1 %exitcond1, label %18, label %10

18:
  tail call cc 1002 void @report_(i32 inreg noundef %16)
  ret i32 0
}

declare cc 1002 void @report_(i32 inreg noundef) "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87"

declare void @llvm.memcpy.p0.p0.i32(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i32, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

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
!9 = !{!"long", !6, i64 0}
