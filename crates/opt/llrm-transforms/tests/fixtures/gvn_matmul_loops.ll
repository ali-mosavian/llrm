target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

define cc 1003 i32 @bench_matmul(i16 noundef zeroext %0) {
  %2 = alloca [8 x [8 x i16]], align 2
  %3 = alloca [8 x [8 x i16]], align 2
  %4 = alloca [8 x [8 x i32]], align 4
  call cc 1003 void @llvm.lifetime.start.p0(i64 -1, ptr %2)
  call cc 1003 void @llvm.lifetime.start.p0(i64 -1, ptr %3)
  call cc 1003 void @llvm.lifetime.start.p0(i64 -1, ptr %4)
  br label %5

5:
  %6 = phi i16 [ 0, %1 ], [ %34, %33 ]
  %7 = zext i16 %6 to i32
  %8 = icmp slt i32 %7, 8
  br i1 %8, label %9, label %35

9:
  br label %10

10:
  %11 = phi i16 [ 0, %9 ], [ %32, %27 ]
  %12 = zext i16 %11 to i32
  %13 = icmp slt i32 %12, 8
  br i1 %13, label %14, label %33

14:
  %15 = mul nsw i32 %7, 3
  %16 = add nsw i32 %15, %12
  %17 = add nsw i32 %16, 1
  %18 = zext i16 %0 to i32
  %19 = add nsw i32 %17, %18
  %20 = trunc i32 %19 to i16
  %21 = getelementptr inbounds nuw [8 x [8 x i16]], ptr %2, i32 0, i32 %7
  %22 = getelementptr inbounds nuw [8 x i16], ptr %21, i32 0, i32 %12
  store i16 %20, ptr %22, align 2
  %23 = icmp eq i32 %7, %12
  br i1 %23, label %27, label %24

24:
  %25 = add nsw i32 %7, %12
  %26 = srem i32 %25, 3
  br label %27

27:
  %28 = phi i32 [ %26, %24 ], [ 2, %14 ]
  %29 = trunc i32 %28 to i16
  %30 = getelementptr inbounds nuw [8 x [8 x i16]], ptr %3, i32 0, i32 %7
  %31 = getelementptr inbounds nuw [8 x i16], ptr %30, i32 0, i32 %12
  store i16 %29, ptr %31, align 2
  %32 = add i16 %11, 1
  br label %10

33:
  %34 = add i16 %6, 1
  br label %5

35:
  br label %36

36:
  %37 = phi i16 [ 0, %35 ], [ %68, %67 ]
  %38 = zext i16 %37 to i32
  %39 = icmp slt i32 %38, 8
  br i1 %39, label %40, label %69

40:
  br label %41

41:
  %42 = phi i16 [ 0, %40 ], [ %66, %63 ]
  %43 = zext i16 %42 to i32
  %44 = icmp slt i32 %43, 8
  br i1 %44, label %45, label %67

45:
  br label %46

46:
  %47 = phi i16 [ 0, %45 ], [ %62, %51 ]
  %48 = phi i32 [ 0, %45 ], [ %61, %51 ]
  %49 = zext i16 %47 to i32
  %50 = icmp slt i32 %49, 8
  br i1 %50, label %51, label %63

51:
  %52 = getelementptr inbounds nuw [8 x [8 x i16]], ptr %2, i32 0, i32 %38
  %53 = getelementptr inbounds nuw [8 x i16], ptr %52, i32 0, i32 %49
  %54 = load i16, ptr %53, align 2
  %55 = sext i16 %54 to i32
  %56 = getelementptr inbounds nuw [8 x [8 x i16]], ptr %3, i32 0, i32 %49
  %57 = getelementptr inbounds nuw [8 x i16], ptr %56, i32 0, i32 %43
  %58 = load i16, ptr %57, align 2
  %59 = sext i16 %58 to i32
  %60 = mul nsw i32 %55, %59
  %61 = add nsw i32 %48, %60
  %62 = add i16 %47, 1
  br label %46

63:
  %64 = getelementptr inbounds nuw [8 x [8 x i32]], ptr %4, i32 0, i32 %38
  %65 = getelementptr inbounds nuw [8 x i32], ptr %64, i32 0, i32 %43
  store i32 %48, ptr %65, align 4
  %66 = add i16 %42, 1
  br label %41

67:
  %68 = add i16 %37, 1
  br label %36

69:
  br label %70

70:
  %71 = phi i16 [ 0, %69 ], [ %92, %91 ]
  %72 = phi i32 [ 0, %69 ], [ %78, %91 ]
  %73 = zext i16 %71 to i32
  %74 = icmp slt i32 %73, 8
  br i1 %74, label %75, label %93

75:
  br label %76

76:
  %77 = phi i16 [ 0, %75 ], [ %90, %81 ]
  %78 = phi i32 [ %72, %75 ], [ %89, %81 ]
  %79 = zext i16 %77 to i32
  %80 = icmp slt i32 %79, 8
  br i1 %80, label %81, label %91

81:
  %82 = getelementptr inbounds nuw [8 x [8 x i32]], ptr %4, i32 0, i32 %73
  %83 = getelementptr inbounds nuw [8 x i32], ptr %82, i32 0, i32 %79
  %84 = load i32, ptr %83, align 4
  %85 = mul nsw i32 %73, 8
  %86 = add nsw i32 %85, %79
  %87 = add nsw i32 %86, 1
  %88 = mul i32 %84, %87
  %89 = add i32 %78, %88
  %90 = add i16 %77, 1
  br label %76

91:
  %92 = add i16 %71, 1
  br label %70

93:
  call cc 1003 void @llvm.lifetime.end.p0(i64 -1, ptr %4)
  call cc 1003 void @llvm.lifetime.end.p0(i64 -1, ptr %3)
  call cc 1003 void @llvm.lifetime.end.p0(i64 -1, ptr %2)
  ret i32 %72
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture)
