target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

@main.xs = internal constant [3 x i32] [i32 19088743, i32 -2147483647, i32 -1], align 4

@__main_void = hidden alias i32 (), ptr @main

define hidden i32 @main() nounwind {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %2) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %3) nounwind
  store i32 0, ptr %2, align 4, !tbaa !6
  br label %4

4:
  %5 = load i32, ptr %2, align 4, !tbaa !6
  %6 = icmp slt i32 %5, 3
  br i1 %6, label %7, label %71

7:
  store i32 0, ptr %3, align 4, !tbaa !6
  br label %8

8:
  %9 = load i32, ptr %3, align 4, !tbaa !6
  %10 = icmp slt i32 %9, 32
  br i1 %10, label %11, label %67

11:
  %12 = load i32, ptr %3, align 4, !tbaa !6
  store volatile i32 %12, ptr %1, align 4, !tbaa !6
  %13 = load i32, ptr %3, align 4, !tbaa !6
  %14 = and i32 %13, 1
  %15 = icmp ne i32 %14, 0
  br i1 %15, label %16, label %22

16:
  %17 = load i32, ptr %2, align 4, !tbaa !6
  %18 = getelementptr inbounds [3 x i32], ptr @main.xs, i32 0, i32 %17
  %19 = load i32, ptr %18, align 4, !tbaa !6
  %20 = load volatile i32, ptr %1, align 4, !tbaa !6
  %21 = call i32 @shl2(i32 noundef %19, i32 noundef %20) nobuiltin "no-builtins"
  br label %28

22:
  %23 = load i32, ptr %2, align 4, !tbaa !6
  %24 = getelementptr inbounds [3 x i32], ptr @main.xs, i32 0, i32 %23
  %25 = load i32, ptr %24, align 4, !tbaa !6
  %26 = load volatile i32, ptr %1, align 4, !tbaa !6
  %27 = call i32 @shl(i32 noundef %25, i32 noundef %26) nobuiltin "no-builtins"
  br label %28

28:
  %29 = phi i32 [ %21, %16 ], [ %27, %22 ]
  call void @report(i32 noundef %29) nobuiltin "no-builtins"
  %30 = load i32, ptr %3, align 4, !tbaa !6
  %31 = and i32 %30, 1
  %32 = icmp ne i32 %31, 0
  br i1 %32, label %33, label %39

33:
  %34 = load i32, ptr %2, align 4, !tbaa !6
  %35 = getelementptr inbounds [3 x i32], ptr @main.xs, i32 0, i32 %34
  %36 = load i32, ptr %35, align 4, !tbaa !6
  %37 = load volatile i32, ptr %1, align 4, !tbaa !6
  %38 = call i32 @shr2(i32 noundef %36, i32 noundef %37) nobuiltin "no-builtins"
  br label %45

39:
  %40 = load i32, ptr %2, align 4, !tbaa !6
  %41 = getelementptr inbounds [3 x i32], ptr @main.xs, i32 0, i32 %40
  %42 = load i32, ptr %41, align 4, !tbaa !6
  %43 = load volatile i32, ptr %1, align 4, !tbaa !6
  %44 = call i32 @shr(i32 noundef %42, i32 noundef %43) nobuiltin "no-builtins"
  br label %45

45:
  %46 = phi i32 [ %38, %33 ], [ %44, %39 ]
  call void @report(i32 noundef %46) nobuiltin "no-builtins"
  %47 = load i32, ptr %3, align 4, !tbaa !6
  %48 = and i32 %47, 1
  %49 = icmp ne i32 %48, 0
  br i1 %49, label %50, label %56

50:
  %51 = load i32, ptr %2, align 4, !tbaa !6
  %52 = getelementptr inbounds [3 x i32], ptr @main.xs, i32 0, i32 %51
  %53 = load i32, ptr %52, align 4, !tbaa !6
  %54 = load volatile i32, ptr %1, align 4, !tbaa !6
  %55 = call i32 @sar2(i32 noundef %53, i32 noundef %54) nobuiltin "no-builtins"
  br label %62

56:
  %57 = load i32, ptr %2, align 4, !tbaa !6
  %58 = getelementptr inbounds [3 x i32], ptr @main.xs, i32 0, i32 %57
  %59 = load i32, ptr %58, align 4, !tbaa !6
  %60 = load volatile i32, ptr %1, align 4, !tbaa !6
  %61 = call i32 @sar(i32 noundef %59, i32 noundef %60) nobuiltin "no-builtins"
  br label %62

62:
  %63 = phi i32 [ %55, %50 ], [ %61, %56 ]
  call void @report(i32 noundef %63) nobuiltin "no-builtins"
  br label %64

64:
  %65 = load i32, ptr %3, align 4, !tbaa !6
  %66 = add nsw i32 %65, 1
  store i32 %66, ptr %3, align 4, !tbaa !6
  br label %8, !llvm.loop !7

67:
  br label %68

68:
  %69 = load i32, ptr %2, align 4, !tbaa !6
  %70 = add nsw i32 %69, 1
  store i32 %70, ptr %2, align 4, !tbaa !6
  br label %4, !llvm.loop !9

71:
  call void @llvm.lifetime.end.p0(i64 -1, ptr %3) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %1) nounwind
  ret i32 0
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture)

declare void @report(i32 noundef)

define internal i32 @shl2(i32 noundef %0, i32 noundef %1) noinline nounwind {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !6
  store i32 %1, ptr %4, align 4, !tbaa !6
  %5 = load i32, ptr %3, align 4, !tbaa !6
  %6 = load i32, ptr %4, align 4, !tbaa !6
  %7 = shl i32 %5, %6
  ret i32 %7
}

define internal i32 @shl(i32 noundef %0, i32 noundef %1) noinline nounwind {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !6
  store i32 %1, ptr %4, align 4, !tbaa !6
  %5 = load i32, ptr %3, align 4, !tbaa !6
  %6 = load i32, ptr %4, align 4, !tbaa !6
  %7 = shl i32 %5, %6
  ret i32 %7
}

define internal i32 @shr2(i32 noundef %0, i32 noundef %1) noinline nounwind {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !6
  store i32 %1, ptr %4, align 4, !tbaa !6
  %5 = load i32, ptr %3, align 4, !tbaa !6
  %6 = load i32, ptr %4, align 4, !tbaa !6
  %7 = lshr i32 %5, %6
  ret i32 %7
}

define internal i32 @shr(i32 noundef %0, i32 noundef %1) noinline nounwind {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !6
  store i32 %1, ptr %4, align 4, !tbaa !6
  %5 = load i32, ptr %3, align 4, !tbaa !6
  %6 = load i32, ptr %4, align 4, !tbaa !6
  %7 = lshr i32 %5, %6
  ret i32 %7
}

define internal i32 @sar2(i32 noundef %0, i32 noundef %1) noinline nounwind {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !6
  store i32 %1, ptr %4, align 4, !tbaa !6
  %5 = load i32, ptr %3, align 4, !tbaa !6
  %6 = load i32, ptr %4, align 4, !tbaa !6
  %7 = ashr i32 %5, %6
  ret i32 %7
}

define internal i32 @sar(i32 noundef %0, i32 noundef %1) noinline nounwind {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !6
  store i32 %1, ptr %4, align 4, !tbaa !6
  %5 = load i32, ptr %3, align 4, !tbaa !6
  %6 = load i32, ptr %4, align 4, !tbaa !6
  %7 = ashr i32 %5, %6
  ret i32 %7
}

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture)

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!3, !3, i64 0}
!7 = distinct !{!7, !8}
!8 = !{!"llvm.loop.mustprogress"}
!9 = distinct !{!9, !8}
