target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

define i64 @f(ptr %0, i64 %1, i64 %2, i64 %3, i64 %4, i64 %5, i64 %6, i64 %7) {
  %9 = load i64, ptr %0, align 8
  %10 = mul nsw i64 %9, 181
  %11 = sdiv i64 %10, 512
  %12 = add nsw i64 %11, 131584
  %13 = getelementptr inbounds nuw i8, ptr %0, i64 64
  %14 = load i64, ptr %13, align 8
  %15 = mul nsw i64 %14, %1
  %16 = sdiv i64 %15, 1024
  %17 = add nsw i64 %12, %16
  %18 = getelementptr inbounds nuw i8, ptr %0, i64 128
  %19 = load i64, ptr %18, align 8
  %20 = mul nsw i64 %19, %2
  %21 = sdiv i64 %20, 1024
  %22 = add nsw i64 %17, %21
  %23 = getelementptr inbounds nuw i8, ptr %0, i64 192
  %24 = load i64, ptr %23, align 8
  %25 = mul nsw i64 %24, %3
  %26 = sdiv i64 %25, 1024
  %27 = add nsw i64 %22, %26
  %28 = getelementptr inbounds nuw i8, ptr %0, i64 256
  %29 = load i64, ptr %28, align 8
  %30 = mul nsw i64 %29, %4
  %31 = sdiv i64 %30, 1024
  %32 = add nsw i64 %27, %31
  %33 = getelementptr inbounds nuw i8, ptr %0, i64 320
  %34 = load i64, ptr %33, align 8
  %35 = mul nsw i64 %34, %5
  %36 = sdiv i64 %35, 1024
  %37 = add nsw i64 %32, %36
  %38 = getelementptr inbounds nuw i8, ptr %0, i64 384
  %39 = load i64, ptr %38, align 8
  %40 = mul nsw i64 %39, %6
  %41 = sdiv i64 %40, 1024
  %42 = add nsw i64 %37, %41
  %43 = getelementptr inbounds nuw i8, ptr %0, i64 448
  %44 = load i64, ptr %43, align 8
  %45 = mul nsw i64 %44, %7
  %46 = sdiv i64 %45, 1024
  %47 = add nsw i64 %42, %46
  %48 = sdiv i64 %47, 1024
  ret i64 %48
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)


