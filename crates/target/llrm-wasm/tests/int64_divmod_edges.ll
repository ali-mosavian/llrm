target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

@E = internal unnamed_addr constant [24 x i64] [i64 0, i64 1, i64 -1, i64 2, i64 -2, i64 3, i64 7, i64 10, i64 255, i64 256, i64 65535, i64 65536, i64 2147483647, i64 2147483648, i64 4294967295, i64 4294967296, i64 9223372036854775807, i64 -9223372036854775808, i64 -9223372036854775807, i64 4611686018427387904, i64 71777214294589695, i64 -4294967296, i64 1311768467463790320, i64 -81985529216486895], align 16


define hidden i32 @main() nounwind {
  %1 = alloca i64, align 8
  %2 = alloca i64, align 8
  %3 = alloca [4 x i32], align 16
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i64, align 8
  %8 = alloca i64, align 8
  %9 = alloca i64, align 8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %2) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %3) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %4) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %5) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %6) nounwind
  store i32 0, ptr %4, align 4
  br label %10

10:
  %11 = load i32, ptr %4, align 4
  %12 = icmp slt i32 %11, 24
  br i1 %12, label %13, label %49

13:
  %14 = getelementptr inbounds [4 x i32], ptr %3, i32 0, i32 3
  store i32 0, ptr %14, align 4
  %15 = getelementptr inbounds [4 x i32], ptr %3, i32 0, i32 2
  store i32 0, ptr %15, align 8
  %16 = getelementptr inbounds [4 x i32], ptr %3, i32 0, i32 1
  store i32 0, ptr %16, align 4
  %17 = getelementptr inbounds [4 x i32], ptr %3, i32 0, i32 0
  store i32 0, ptr %17, align 16
  store i32 0, ptr %5, align 4
  br label %18

18:
  %19 = load i32, ptr %5, align 4
  %20 = icmp slt i32 %19, 24
  br i1 %20, label %21, label %34

21:
  %22 = load i32, ptr %4, align 4
  %23 = getelementptr inbounds [24 x i64], ptr @E, i32 0, i32 %22
  %24 = load i64, ptr %23, align 8
  store volatile i64 %24, ptr %1, align 8
  %25 = load i32, ptr %5, align 4
  %26 = getelementptr inbounds [24 x i64], ptr @E, i32 0, i32 %25
  %27 = load i64, ptr %26, align 8
  store volatile i64 %27, ptr %2, align 8
  %28 = getelementptr inbounds [4 x i32], ptr %3, i32 0, i32 0
  %29 = load volatile i64, ptr %1, align 8
  %30 = load volatile i64, ptr %2, align 8
  call void @pair(ptr noundef %28, i64 noundef %29, i64 noundef %30) "no-builtins"
  br label %31

31:
  %32 = load i32, ptr %5, align 4
  %33 = add nsw i32 %32, 1
  store i32 %33, ptr %5, align 4
  br label %18

34:
  store i32 0, ptr %6, align 4
  br label %35

35:
  %36 = load i32, ptr %6, align 4
  %37 = icmp slt i32 %36, 4
  br i1 %37, label %38, label %45

38:
  %39 = load i32, ptr %6, align 4
  %40 = getelementptr inbounds [4 x i32], ptr %3, i32 0, i32 %39
  %41 = load i32, ptr %40, align 4
  call void @report(i32 noundef %41) "no-builtins"
  br label %42

42:
  %43 = load i32, ptr %6, align 4
  %44 = add nsw i32 %43, 1
  store i32 %44, ptr %6, align 4
  br label %35

45:
  br label %46

46:
  %47 = load i32, ptr %4, align 4
  %48 = add nsw i32 %47, 1
  store i32 %48, ptr %4, align 4
  br label %10

49:
  call void @llvm.lifetime.start.p0(i64 -1, ptr %7) nounwind
  store i64 88172645463325252, ptr %7, align 8
  store i32 0, ptr %4, align 4
  br label %50

50:
  %51 = load i32, ptr %4, align 4
  %52 = icmp slt i32 %51, 4
  br i1 %52, label %53, label %101

53:
  %54 = getelementptr inbounds [4 x i32], ptr %3, i32 0, i32 3
  store i32 0, ptr %54, align 4
  %55 = getelementptr inbounds [4 x i32], ptr %3, i32 0, i32 2
  store i32 0, ptr %55, align 8
  %56 = getelementptr inbounds [4 x i32], ptr %3, i32 0, i32 1
  store i32 0, ptr %56, align 4
  %57 = getelementptr inbounds [4 x i32], ptr %3, i32 0, i32 0
  store i32 0, ptr %57, align 16
  store i32 0, ptr %5, align 4
  br label %58

58:
  %59 = load i32, ptr %5, align 4
  %60 = icmp slt i32 %59, 25
  br i1 %60, label %61, label %86

61:
  %62 = load i64, ptr %7, align 8
  %63 = mul i64 %62, 6364136223846793005
  %64 = add i64 %63, 1442695040888963407
  store i64 %64, ptr %7, align 8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %8) nounwind
  %65 = load i64, ptr %7, align 8
  %66 = load i64, ptr %7, align 8
  %67 = lshr i64 %66, 58
  %68 = and i64 %67, 63
  %69 = lshr i64 %65, %68
  store i64 %69, ptr %8, align 8
  %70 = load i64, ptr %7, align 8
  %71 = mul i64 %70, 6364136223846793005
  %72 = add i64 %71, 1442695040888963407
  store i64 %72, ptr %7, align 8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %9) nounwind
  %73 = load i64, ptr %7, align 8
  %74 = load i64, ptr %7, align 8
  %75 = lshr i64 %74, 58
  %76 = and i64 %75, 63
  %77 = lshr i64 %73, %76
  store i64 %77, ptr %9, align 8
  %78 = load i64, ptr %8, align 8
  store volatile i64 %78, ptr %1, align 8
  %79 = load i64, ptr %9, align 8
  store volatile i64 %79, ptr %2, align 8
  %80 = getelementptr inbounds [4 x i32], ptr %3, i32 0, i32 0
  %81 = load volatile i64, ptr %1, align 8
  %82 = load volatile i64, ptr %2, align 8
  call void @pair(ptr noundef %80, i64 noundef %81, i64 noundef %82) "no-builtins"
  call void @llvm.lifetime.end.p0(i64 -1, ptr %9) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %8) nounwind
  br label %83

83:
  %84 = load i32, ptr %5, align 4
  %85 = add nsw i32 %84, 1
  store i32 %85, ptr %5, align 4
  br label %58

86:
  store i32 0, ptr %6, align 4
  br label %87

87:
  %88 = load i32, ptr %6, align 4
  %89 = icmp slt i32 %88, 4
  br i1 %89, label %90, label %97

90:
  %91 = load i32, ptr %6, align 4
  %92 = getelementptr inbounds [4 x i32], ptr %3, i32 0, i32 %91
  %93 = load i32, ptr %92, align 4
  call void @report(i32 noundef %93) "no-builtins"
  br label %94

94:
  %95 = load i32, ptr %6, align 4
  %96 = add nsw i32 %95, 1
  store i32 %96, ptr %6, align 4
  br label %87

97:
  br label %98

98:
  %99 = load i32, ptr %4, align 4
  %100 = add nsw i32 %99, 1
  store i32 %100, ptr %4, align 4
  br label %50

101:
  call void @llvm.lifetime.end.p0(i64 -1, ptr %7) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %6) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %5) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %4) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %3) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %1) nounwind
  ret i32 0
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal void @pair(ptr noundef %0, i64 noundef %1, i64 noundef %2) nounwind {
  %4 = alloca ptr, align 4
  %5 = alloca i64, align 8
  %6 = alloca i64, align 8
  store ptr %0, ptr %4, align 4
  store i64 %1, ptr %5, align 8
  store i64 %2, ptr %6, align 8
  %7 = load i64, ptr %6, align 8
  %8 = icmp eq i64 %7, 0
  br i1 %8, label %15, label %9

9:
  %10 = load i64, ptr %5, align 8
  %11 = icmp eq i64 %10, -9223372036854775808
  br i1 %11, label %12, label %16

12:
  %13 = load i64, ptr %6, align 8
  %14 = icmp eq i64 %13, -1
  br i1 %14, label %15, label %16

15:
  br label %53

16:
  %17 = load ptr, ptr %4, align 4
  %18 = getelementptr inbounds i32, ptr %17, i32 0
  %19 = load i32, ptr %18, align 4
  %20 = load i64, ptr %5, align 8
  %21 = load i64, ptr %6, align 8
  %22 = sdiv i64 %20, %21
  %23 = call i32 @mix(i32 noundef %19, i64 noundef %22) "no-builtins"
  %24 = load ptr, ptr %4, align 4
  %25 = getelementptr inbounds i32, ptr %24, i32 0
  store i32 %23, ptr %25, align 4
  %26 = load ptr, ptr %4, align 4
  %27 = getelementptr inbounds i32, ptr %26, i32 1
  %28 = load i32, ptr %27, align 4
  %29 = load i64, ptr %5, align 8
  %30 = load i64, ptr %6, align 8
  %31 = srem i64 %29, %30
  %32 = call i32 @mix(i32 noundef %28, i64 noundef %31) "no-builtins"
  %33 = load ptr, ptr %4, align 4
  %34 = getelementptr inbounds i32, ptr %33, i32 1
  store i32 %32, ptr %34, align 4
  %35 = load ptr, ptr %4, align 4
  %36 = getelementptr inbounds i32, ptr %35, i32 2
  %37 = load i32, ptr %36, align 4
  %38 = load i64, ptr %5, align 8
  %39 = load i64, ptr %6, align 8
  %40 = udiv i64 %38, %39
  %41 = call i32 @mix(i32 noundef %37, i64 noundef %40) "no-builtins"
  %42 = load ptr, ptr %4, align 4
  %43 = getelementptr inbounds i32, ptr %42, i32 2
  store i32 %41, ptr %43, align 4
  %44 = load ptr, ptr %4, align 4
  %45 = getelementptr inbounds i32, ptr %44, i32 3
  %46 = load i32, ptr %45, align 4
  %47 = load i64, ptr %5, align 8
  %48 = load i64, ptr %6, align 8
  %49 = urem i64 %47, %48
  %50 = call i32 @mix(i32 noundef %46, i64 noundef %49) "no-builtins"
  %51 = load ptr, ptr %4, align 4
  %52 = getelementptr inbounds i32, ptr %51, i32 3
  store i32 %50, ptr %52, align 4
  br label %53

53:
  ret void
}

declare void @report(i32 noundef)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal i32 @mix(i32 noundef %0, i64 noundef %1) nounwind {
  %3 = alloca i32, align 4
  %4 = alloca i64, align 8
  store i32 %0, ptr %3, align 4
  store i64 %1, ptr %4, align 8
  %5 = load i32, ptr %3, align 4
  %6 = mul i32 %5, 31
  %7 = load i64, ptr %4, align 8
  %8 = trunc i64 %7 to i32
  %9 = add i32 %6, %8
  %10 = load i64, ptr %4, align 8
  %11 = lshr i64 %10, 32
  %12 = trunc i64 %11 to i32
  %13 = mul i32 %12, 7
  %14 = add i32 %9, %13
  ret i32 %14
}

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!3, !3, i64 0}
!7 = !{!8, !8, i64 0}
!8 = !{!"long long", !4, i64 0}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
!11 = distinct !{!11, !10}
!12 = distinct !{!12, !10}
!13 = distinct !{!13, !10}
!14 = distinct !{!14, !10}
!15 = distinct !{!15, !10}
!16 = !{!17, !17, i64 0}
!17 = !{!"p1 int", !18, i64 0}
!18 = !{!"any pointer", !4, i64 0}
