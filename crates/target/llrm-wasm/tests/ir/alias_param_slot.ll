target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture)
declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture)
declare fastcc void @ref_copy(ptr,ptr,i32)
declare fastcc void @ref_move(ptr,ptr,i32)
declare fastcc void @ref_fill(ptr,i32,i32)
define void @lr9(ptr noundef nocapture nofree writeonly %0, ptr noundef nocapture nofree readonly %1) noinline nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" nofree nosync norecurse memory(argmem: readwrite, inaccessiblemem: readwrite) willreturn {
  %3 = alloca [700 x i8], align 16
  %4 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %3) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %4) nounwind
  br label %11

5:
  %6 = getelementptr inbounds nuw i8, ptr %4, i32 3
  %7 = getelementptr inbounds nuw i8, ptr %3, i32 1
  call fastcc void @ref_copy(ptr noundef nonnull %6, ptr noundef nonnull %7, i32 noundef 9) nobuiltin "no-builtins"
  %8 = getelementptr inbounds nuw i8, ptr %3, i32 5
  %9 = getelementptr inbounds nuw i8, ptr %3, i32 2
  call fastcc void @ref_move(ptr noundef nonnull %8, ptr noundef nonnull %9, i32 noundef 9) nobuiltin "no-builtins"
  %10 = getelementptr inbounds nuw i8, ptr %4, i32 650
  call fastcc void @ref_fill(ptr noundef nonnull %10, i32 noundef 51, i32 noundef 9) nobuiltin "no-builtins"
  br label %35

11:
  %12 = phi i32 [ 0, %2 ], [ %32, %11 ]
  %13 = getelementptr inbounds nuw i8, ptr %1, i32 %12
  %14 = load i8, ptr %13, align 1
  %15 = getelementptr inbounds nuw i8, ptr %3, i32 %12
  store i8 %14, ptr %15, align 4
  %16 = getelementptr inbounds nuw i8, ptr %4, i32 %12
  store i8 -18, ptr %16, align 4
  %17 = or disjoint i32 %12, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i32 %17
  %19 = load i8, ptr %18, align 1
  %20 = getelementptr inbounds nuw i8, ptr %3, i32 %17
  store i8 %19, ptr %20, align 1
  %21 = getelementptr inbounds nuw i8, ptr %4, i32 %17
  store i8 -18, ptr %21, align 1
  %22 = or disjoint i32 %12, 2
  %23 = getelementptr inbounds nuw i8, ptr %1, i32 %22
  %24 = load i8, ptr %23, align 1
  %25 = getelementptr inbounds nuw i8, ptr %3, i32 %22
  store i8 %24, ptr %25, align 2
  %26 = getelementptr inbounds nuw i8, ptr %4, i32 %22
  store i8 -18, ptr %26, align 2
  %27 = or disjoint i32 %12, 3
  %28 = getelementptr inbounds nuw i8, ptr %1, i32 %27
  %29 = load i8, ptr %28, align 1
  %30 = getelementptr inbounds nuw i8, ptr %3, i32 %27
  store i8 %29, ptr %30, align 1
  %31 = getelementptr inbounds nuw i8, ptr %4, i32 %27
  store i8 -18, ptr %31, align 1
  %32 = add nuw nsw i32 %12, 4
  %33 = icmp eq i32 %32, 700
  br i1 %33, label %5, label %11

34:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %4) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %3) nounwind
  ret void

35:
  %36 = phi i32 [ 0, %5 ], [ %50, %35 ]
  %37 = getelementptr inbounds nuw i8, ptr %3, i32 %36
  %38 = load i8, ptr %37, align 2
  %39 = getelementptr inbounds nuw i8, ptr %4, i32 %36
  %40 = load i8, ptr %39, align 2
  %41 = xor i8 %40, %38
  %42 = getelementptr inbounds nuw i8, ptr %0, i32 %36
  store i8 %41, ptr %42, align 1
  %43 = or disjoint i32 %36, 1
  %44 = getelementptr inbounds nuw i8, ptr %3, i32 %43
  %45 = load i8, ptr %44, align 1
  %46 = getelementptr inbounds nuw i8, ptr %4, i32 %43
  %47 = load i8, ptr %46, align 1
  %48 = xor i8 %47, %45
  %49 = getelementptr inbounds nuw i8, ptr %0, i32 %43
  store i8 %48, ptr %49, align 1
  %50 = add nuw nsw i32 %36, 2
  %51 = icmp eq i32 %50, 700
  br i1 %51, label %34, label %35
}
