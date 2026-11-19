source_filename = "g.c"
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

@px = hidden global [6 x i32] zeroinitializer, align 16
@py = hidden global [6 x i32] zeroinitializer, align 16
@vx = hidden global [6 x i32] zeroinitializer, align 16
@vy = hidden global [6 x i32] zeroinitializer, align 16

define hidden void @g() {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  %8 = alloca i32, align 4
  %9 = alloca i32, align 4
  call void @llvm.lifetime.start.p0(ptr %1)
  store i32 0, ptr %1, align 4
  br label %10

10:                                               ; preds = %85, %0
  %11 = load i32, ptr %1, align 4
  %12 = icmp slt i32 %11, 1000
  br i1 %12, label %14, label %13

13:                                               ; preds = %10
  store i32 2, ptr %2, align 4
  call void @llvm.lifetime.end.p0(ptr %1)
  br label %88

14:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %3)
  store i32 0, ptr %3, align 4
  br label %15

15:                                               ; preds = %81, %14
  %16 = load i32, ptr %3, align 4
  %17 = icmp slt i32 %16, 6
  br i1 %17, label %19, label %18

18:                                               ; preds = %15
  store i32 5, ptr %2, align 4
  call void @llvm.lifetime.end.p0(ptr %3)
  br label %84

19:                                               ; preds = %15
  call void @llvm.lifetime.start.p0(ptr %4)
  store i32 0, ptr %4, align 4
  call void @llvm.lifetime.start.p0(ptr %5)
  store i32 0, ptr %5, align 4
  call void @llvm.lifetime.start.p0(ptr %6)
  store i32 0, ptr %6, align 4
  br label %20

20:                                               ; preds = %67, %19
  %21 = load i32, ptr %6, align 4
  %22 = icmp slt i32 %21, 6
  br i1 %22, label %24, label %23

23:                                               ; preds = %20
  store i32 8, ptr %2, align 4
  call void @llvm.lifetime.end.p0(ptr %6)
  br label %70

24:                                               ; preds = %20
  %25 = load i32, ptr %6, align 4
  %26 = load i32, ptr %3, align 4
  %27 = icmp ne i32 %25, %26
  br i1 %27, label %28, label %66

28:                                               ; preds = %24
  call void @llvm.lifetime.start.p0(ptr %7)
  %29 = load i32, ptr %6, align 4
  %30 = getelementptr inbounds [6 x i32], ptr @px, i32 0, i32 %29
  %31 = load i32, ptr %30, align 4
  %32 = load i32, ptr %3, align 4
  %33 = getelementptr inbounds [6 x i32], ptr @px, i32 0, i32 %32
  %34 = load i32, ptr %33, align 4
  %35 = sub nsw i32 %31, %34
  store i32 %35, ptr %7, align 4
  call void @llvm.lifetime.start.p0(ptr %8)
  %36 = load i32, ptr %6, align 4
  %37 = getelementptr inbounds [6 x i32], ptr @py, i32 0, i32 %36
  %38 = load i32, ptr %37, align 4
  %39 = load i32, ptr %3, align 4
  %40 = getelementptr inbounds [6 x i32], ptr @py, i32 0, i32 %39
  %41 = load i32, ptr %40, align 4
  %42 = sub nsw i32 %38, %41
  store i32 %42, ptr %8, align 4
  call void @llvm.lifetime.start.p0(ptr %9)
  %43 = load i32, ptr %7, align 4
  %44 = load i32, ptr %7, align 4
  %45 = mul nsw i32 %43, %44
  %46 = load i32, ptr %8, align 4
  %47 = load i32, ptr %8, align 4
  %48 = mul nsw i32 %46, %47
  %49 = add nsw i32 %45, %48
  %50 = add nsw i32 %49, 262144
  %51 = sdiv i32 %50, 262144
  %52 = add nsw i32 %51, 1
  %53 = sdiv i32 512, %52
  store i32 %53, ptr %9, align 4
  %54 = load i32, ptr %7, align 4
  %55 = load i32, ptr %9, align 4
  %56 = mul nsw i32 %54, %55
  %57 = sdiv i32 %56, 512
  %58 = load i32, ptr %4, align 4
  %59 = add nsw i32 %58, %57
  store i32 %59, ptr %4, align 4
  %60 = load i32, ptr %8, align 4
  %61 = load i32, ptr %9, align 4
  %62 = mul nsw i32 %60, %61
  %63 = sdiv i32 %62, 512
  %64 = load i32, ptr %5, align 4
  %65 = add nsw i32 %64, %63
  store i32 %65, ptr %5, align 4
  call void @llvm.lifetime.end.p0(ptr %9)
  call void @llvm.lifetime.end.p0(ptr %8)
  call void @llvm.lifetime.end.p0(ptr %7)
  br label %66

66:                                               ; preds = %28, %24
  br label %67

67:                                               ; preds = %66
  %68 = load i32, ptr %6, align 4
  %69 = add nsw i32 %68, 1
  store i32 %69, ptr %6, align 4
  br label %20

70:                                               ; preds = %23
  %71 = load i32, ptr %4, align 4
  %72 = load i32, ptr %3, align 4
  %73 = getelementptr inbounds [6 x i32], ptr @vx, i32 0, i32 %72
  %74 = load i32, ptr %73, align 4
  %75 = add nsw i32 %74, %71
  store i32 %75, ptr %73, align 4
  %76 = load i32, ptr %5, align 4
  %77 = load i32, ptr %3, align 4
  %78 = getelementptr inbounds [6 x i32], ptr @vy, i32 0, i32 %77
  %79 = load i32, ptr %78, align 4
  %80 = add nsw i32 %79, %76
  store i32 %80, ptr %78, align 4
  call void @llvm.lifetime.end.p0(ptr %5)
  call void @llvm.lifetime.end.p0(ptr %4)
  br label %81

81:                                               ; preds = %70
  %82 = load i32, ptr %3, align 4
  %83 = add nsw i32 %82, 1
  store i32 %83, ptr %3, align 4
  br label %15

84:                                               ; preds = %18
  br label %85

85:                                               ; preds = %84
  %86 = load i32, ptr %1, align 4
  %87 = add nsw i32 %86, 1
  store i32 %87, ptr %1, align 4
  br label %10

88:                                               ; preds = %13
  ret void
}

declare void @llvm.lifetime.start.p0(ptr captures(none))

declare void @llvm.lifetime.end.p0(ptr captures(none))

