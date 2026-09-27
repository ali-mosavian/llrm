target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$var_abi.qb45.result = internal global [4 x i8] zeroinitializer

define internal ptr addrspace(1) @abi.basic.array_data(ptr %0) addrspace(1) {
b1:
  %1 = load ptr addrspace(1), ptr %0
  ret ptr addrspace(1) %1
}

define internal i16 @abi.basic.array_count(ptr %0, i16 %1) addrspace(1) {
b1:
  %2 = mul i16 1, 14
  %3 = getelementptr i8, ptr %0, i16 %2
  %4 = mul i16 2, %1
  %5 = mul i16 %4, 2
  %6 = getelementptr i8, ptr %3, i16 %5
  %7 = load i16, ptr %6
  ret i16 %7
}

define internal ptr addrspace(1) @abi.qb45.string_data(ptr %0) addrspace(1) {
b1:
  %1 = alloca ptr
  store ptr null, ptr %1
  %2 = getelementptr i8, ptr %0, i16 2
  %3 = load ptr, ptr %2
  store ptr %3, ptr %1, !tbaa !2
  %4 = load ptr, ptr %1, !tbaa !2
  %5 = addrspacecast ptr %4 to ptr addrspace(1)
  ret ptr addrspace(1) %5
}

define internal i16 @abi.qb45.string_length(ptr %0) addrspace(1) {
b1:
  %1 = load i16, ptr %0
  ret i16 %1
}

define internal ptr @abi.qb45.string_result(ptr addrspace(1) %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca ptr addrspace(1)
  store ptr addrspace(1) null, ptr %2
  store i16 %1, ptr @$var_abi.qb45.result, !tbaa !2
  store ptr addrspace(1) %0, ptr %2, !tbaa !2
  %3 = load ptr, ptr %2, !tbaa !2
  %4 = getelementptr inbounds i8, ptr @$var_abi.qb45.result, i16 2
  store ptr %3, ptr %4, !tbaa !2
  %5 = call cc1000 addrspace(1) ptr @B$SCPY(ptr @$var_abi.qb45.result)
  ret ptr %5
}

define cc1000 void @SORTSCORES(ptr %0, ptr %1) addrspace(1) {
b1:
  %2 = alloca i8
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca [8 x i8]
  store i8 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 8, i1 false)
  %8 = call addrspace(1) ptr addrspace(1) @abi.basic.array_data(ptr %0)
  %9 = call addrspace(1) i16 @abi.basic.array_count(ptr %0, i16 0)
  %10 = addrspacecast ptr %7 to ptr addrspace(1)
  store i16 %9, ptr addrspace(1) %10, !tbaa !2
  %11 = getelementptr i8, ptr addrspace(1) %10, i16 2
  store i16 %9, ptr addrspace(1) %11, !tbaa !2
  %12 = getelementptr i8, ptr addrspace(1) %10, i16 4
  store ptr addrspace(1) %8, ptr addrspace(1) %12, !tbaa !2
  %13 = load i16, ptr %1
  store i16 %13, ptr %6, !tbaa !2
  store i16 1, ptr %5, !tbaa !2
  br label %b2

b2:
  %14 = load i16, ptr %5, !tbaa !2
  %15 = load i16, ptr %6, !tbaa !2
  %16 = icmp ult i16 %14, %15
  %17 = sext i1 %16 to i8
  %18 = icmp ne i8 %17, 0
  br i1 %18, label %b3, label %b4

b3:
  %19 = load i16, ptr %5, !tbaa !2
  %20 = getelementptr i8, ptr addrspace(1) %10, i16 4
  %21 = load ptr addrspace(1), ptr addrspace(1) %20, !tbaa !2
  %22 = mul i16 %19, 2
  %23 = getelementptr i8, ptr addrspace(1) %21, i16 %22
  %24 = load i16, ptr addrspace(1) %23
  store i16 %24, ptr %4, !tbaa !2
  %25 = load i16, ptr %5, !tbaa !2
  store i16 %25, ptr %3, !tbaa !2
  br label %b5

b4:
  ret void

b5:
  %26 = load i16, ptr %3, !tbaa !2
  %27 = icmp ugt i16 %26, 0
  %28 = sext i1 %27 to i8
  store i8 %28, ptr %2, !tbaa !2
  %29 = icmp ne i8 %28, 0
  br i1 %29, label %b8, label %b9

b6:
  %30 = load i16, ptr %3, !tbaa !2
  %31 = getelementptr i8, ptr addrspace(1) %10, i16 4
  %32 = load ptr addrspace(1), ptr addrspace(1) %31, !tbaa !2
  %33 = mul i16 %30, 2
  %34 = getelementptr i8, ptr addrspace(1) %32, i16 %33
  %35 = load i16, ptr %3, !tbaa !2
  %36 = sub i16 %35, 1
  %37 = getelementptr i8, ptr addrspace(1) %10, i16 4
  %38 = load ptr addrspace(1), ptr addrspace(1) %37, !tbaa !2
  %39 = mul i16 %36, 2
  %40 = getelementptr i8, ptr addrspace(1) %38, i16 %39
  %41 = load i16, ptr addrspace(1) %40
  store i16 %41, ptr addrspace(1) %34
  %42 = load i16, ptr %3, !tbaa !2
  %43 = sub i16 %42, 1
  store i16 %43, ptr %3, !tbaa !2
  br label %b5

b7:
  %44 = load i16, ptr %3, !tbaa !2
  %45 = getelementptr i8, ptr addrspace(1) %10, i16 4
  %46 = load ptr addrspace(1), ptr addrspace(1) %45, !tbaa !2
  %47 = mul i16 %44, 2
  %48 = getelementptr i8, ptr addrspace(1) %46, i16 %47
  %49 = load i16, ptr %4, !tbaa !2
  store i16 %49, ptr addrspace(1) %48
  %50 = load i16, ptr %5, !tbaa !2
  %51 = add i16 %50, 1
  store i16 %51, ptr %5, !tbaa !2
  br label %b2

b8:
  %52 = load i16, ptr %3, !tbaa !2
  %53 = sub i16 %52, 1
  %54 = getelementptr i8, ptr addrspace(1) %10, i16 4
  %55 = load ptr addrspace(1), ptr addrspace(1) %54, !tbaa !2
  %56 = mul i16 %53, 2
  %57 = getelementptr i8, ptr addrspace(1) %55, i16 %56
  %58 = load i16, ptr addrspace(1) %57
  %59 = load i16, ptr %4, !tbaa !2
  %60 = icmp slt i16 %58, %59
  %61 = sext i1 %60 to i8
  store i8 %61, ptr %2, !tbaa !2
  br label %b9

b9:
  %62 = load i8, ptr %2, !tbaa !2
  %63 = icmp ne i8 %62, 0
  br i1 %63, label %b6, label %b7
}

define cc1000 void @UPPER(ptr %0) addrspace(1) {
b1:
  %1 = alloca i8
  %2 = alloca i8
  %3 = alloca i16
  %4 = alloca [8 x i8]
  store i8 0, ptr %1
  store i8 0, ptr %2
  store i16 0, ptr %3
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  %5 = call addrspace(1) ptr addrspace(1) @abi.qb45.string_data(ptr %0)
  %6 = call addrspace(1) i16 @abi.qb45.string_length(ptr %0)
  %7 = addrspacecast ptr %4 to ptr addrspace(1)
  store i16 %6, ptr addrspace(1) %7, !tbaa !2
  %8 = getelementptr i8, ptr addrspace(1) %7, i16 2
  store i16 %6, ptr addrspace(1) %8, !tbaa !2
  %9 = getelementptr i8, ptr addrspace(1) %7, i16 4
  store ptr addrspace(1) %5, ptr addrspace(1) %9, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  br label %b2

b2:
  %10 = load i16, ptr %3, !tbaa !2
  %11 = load i16, ptr addrspace(1) %7
  %12 = icmp ult i16 %10, %11
  %13 = sext i1 %12 to i8
  %14 = icmp ne i8 %13, 0
  br i1 %14, label %b3, label %b4

b3:
  %15 = load i16, ptr %3, !tbaa !2
  %16 = getelementptr i8, ptr addrspace(1) %7, i16 4
  %17 = load ptr addrspace(1), ptr addrspace(1) %16, !tbaa !2
  %18 = getelementptr i8, ptr addrspace(1) %17, i16 %15
  %19 = load i8, ptr addrspace(1) %18
  store i8 %19, ptr %2, !tbaa !2
  %20 = load i8, ptr %2, !tbaa !2
  %21 = icmp ule i8 97, %20
  %22 = sext i1 %21 to i8
  store i8 %22, ptr %1, !tbaa !2
  %23 = icmp ne i8 %22, 0
  br i1 %23, label %b6, label %b5

b4:
  ret void

b5:
  %24 = load i8, ptr %1, !tbaa !2
  %25 = icmp ne i8 %24, 0
  br i1 %25, label %b7, label %b8

b6:
  %26 = icmp ule i8 %20, 122
  %27 = sext i1 %26 to i8
  store i8 %27, ptr %1, !tbaa !2
  br label %b5

b7:
  %28 = load i16, ptr %3, !tbaa !2
  %29 = getelementptr i8, ptr addrspace(1) %7, i16 4
  %30 = load ptr addrspace(1), ptr addrspace(1) %29, !tbaa !2
  %31 = getelementptr i8, ptr addrspace(1) %30, i16 %28
  %32 = load i8, ptr %2, !tbaa !2
  %33 = zext i8 %32 to i16
  %34 = sub i16 %33, 32
  %35 = trunc i16 %34 to i8
  store i8 %35, ptr addrspace(1) %31
  br label %b9

b8:
  br label %b9

b9:
  %36 = load i16, ptr %3, !tbaa !2
  %37 = add i16 %36, 1
  store i16 %37, ptr %3, !tbaa !2
  br label %b2
}

define cc1000 double @AVERAGE(ptr %0, ptr %1, ptr %2) addrspace(1) {
b1:
  %3 = alloca double
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca [8 x i8]
  store double 0.000000e+00, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 8, i1 false)
  %8 = call addrspace(1) ptr addrspace(1) @abi.basic.array_data(ptr %0)
  %9 = call addrspace(1) i16 @abi.basic.array_count(ptr %0, i16 0)
  %10 = addrspacecast ptr %7 to ptr addrspace(1)
  store i16 %9, ptr addrspace(1) %10, !tbaa !2
  %11 = getelementptr i8, ptr addrspace(1) %10, i16 2
  store i16 %9, ptr addrspace(1) %11, !tbaa !2
  %12 = getelementptr i8, ptr addrspace(1) %10, i16 4
  store ptr addrspace(1) %8, ptr addrspace(1) %12, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  %13 = load i16, ptr %1
  store i16 %13, ptr %5, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  br label %b2

b2:
  %14 = load i16, ptr %4, !tbaa !2
  %15 = load i16, ptr %5, !tbaa !2
  %16 = icmp ult i16 %14, %15
  %17 = sext i1 %16 to i8
  %18 = icmp ne i8 %17, 0
  br i1 %18, label %b3, label %b4

b3:
  %19 = load i16, ptr %6, !tbaa !2
  %20 = load i16, ptr %4, !tbaa !2
  %21 = getelementptr i8, ptr addrspace(1) %10, i16 4
  %22 = load ptr addrspace(1), ptr addrspace(1) %21, !tbaa !2
  %23 = mul i16 %20, 2
  %24 = getelementptr i8, ptr addrspace(1) %22, i16 %23
  %25 = load i16, ptr addrspace(1) %24
  %26 = add i16 %19, %25
  store i16 %26, ptr %6, !tbaa !2
  %27 = load i16, ptr %4, !tbaa !2
  %28 = add i16 %27, 1
  store i16 %28, ptr %4, !tbaa !2
  br label %b2

b4:
  %29 = call cc1000 addrspace(1) double @MEAN(ptr %6, ptr %5, ptr %3)
  ret double %29
}

define cc1000 i16 @ROWTOTAL(ptr %0, ptr %1) addrspace(1) {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca [10 x i8]
  store i16 0, ptr %2
  store i16 0, ptr %3
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 10, i1 false)
  %5 = call addrspace(1) ptr addrspace(1) @abi.basic.array_data(ptr %0)
  %6 = call addrspace(1) i16 @abi.basic.array_count(ptr %0, i16 0)
  %7 = call addrspace(1) i16 @abi.basic.array_count(ptr %0, i16 1)
  %8 = addrspacecast ptr %4 to ptr addrspace(1)
  store i16 %6, ptr addrspace(1) %8, !tbaa !2
  %9 = getelementptr i8, ptr addrspace(1) %8, i16 2
  store i16 %7, ptr addrspace(1) %9, !tbaa !2
  %10 = mul i16 %6, %7
  %11 = getelementptr i8, ptr addrspace(1) %8, i16 4
  store i16 %10, ptr addrspace(1) %11, !tbaa !2
  %12 = getelementptr i8, ptr addrspace(1) %8, i16 6
  store ptr addrspace(1) %5, ptr addrspace(1) %12, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  br label %b2

b2:
  %13 = load i16, ptr %2, !tbaa !2
  %14 = load i16, ptr addrspace(1) %8, !tbaa !2
  %15 = icmp ult i16 %13, %14
  %16 = sext i1 %15 to i8
  %17 = icmp ne i8 %16, 0
  br i1 %17, label %b3, label %b4

b3:
  %18 = load i16, ptr %3, !tbaa !2
  %19 = load i16, ptr %2, !tbaa !2
  %20 = load i16, ptr %1
  %21 = getelementptr i8, ptr addrspace(1) %8, i16 2
  %22 = load i16, ptr addrspace(1) %21, !tbaa !2
  %23 = mul i16 %22, 1
  %24 = mul i16 %19, %23
  %25 = mul i16 %20, 1
  %26 = add i16 %24, %25
  %27 = getelementptr i8, ptr addrspace(1) %8, i16 6
  %28 = load ptr addrspace(1), ptr addrspace(1) %27, !tbaa !2
  %29 = mul i16 %26, 2
  %30 = getelementptr i8, ptr addrspace(1) %28, i16 %29
  %31 = load i16, ptr addrspace(1) %30
  %32 = add i16 %18, %31
  store i16 %32, ptr %3, !tbaa !2
  %33 = load i16, ptr %2, !tbaa !2
  %34 = add i16 %33, 1
  store i16 %34, ptr %2, !tbaa !2
  br label %b2

b4:
  %35 = load i16, ptr %3, !tbaa !2
  ret i16 %35
}

define cc1000 ptr @INITIALS(ptr %0) addrspace(1) {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca i8
  %3 = alloca i16
  %4 = alloca i8
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca [2 x i8]
  %11 = alloca i8
  %12 = alloca [8 x i8]
  %13 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  store i8 0, ptr %2
  store i16 0, ptr %3
  store i8 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  call void @llvm.memset.p0.i16(ptr %10, i8 0, i16 2, i1 false)
  store i8 0, ptr %11
  call void @llvm.memset.p0.i16(ptr %12, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %13, i8 0, i16 8, i1 false)
  %14 = addrspacecast ptr %13 to ptr addrspace(1)
  %15 = call addrspace(1) ptr addrspace(1) @abi.qb45.string_data(ptr %0)
  %16 = call addrspace(1) i16 @abi.qb45.string_length(ptr %0)
  %17 = addrspacecast ptr %12 to ptr addrspace(1)
  store i16 %16, ptr addrspace(1) %17, !tbaa !2
  %18 = getelementptr i8, ptr addrspace(1) %17, i16 2
  store i16 %16, ptr addrspace(1) %18, !tbaa !2
  %19 = getelementptr i8, ptr addrspace(1) %17, i16 4
  store ptr addrspace(1) %15, ptr addrspace(1) %19, !tbaa !2
  store i8 63, ptr %11, !tbaa !2
  store i16 2, ptr %8, !tbaa !2
  store i16 2, ptr %9, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  store i16 2, ptr %6, !tbaa !2
  br label %b2

b2:
  %20 = load i16, ptr %7, !tbaa !2
  %21 = load i16, ptr %6, !tbaa !2
  %22 = icmp slt i16 %20, %21
  %23 = sext i1 %22 to i8
  %24 = icmp ne i8 %23, 0
  br i1 %24, label %b3, label %b5

b3:
  %25 = load i16, ptr %7, !tbaa !2
  %26 = load i8, ptr %11, !tbaa !2
  %27 = sub i16 %25, 0
  %28 = getelementptr inbounds i8, ptr %10, i16 %27
  store i8 %26, ptr %28, !tbaa !2
  br label %b4

b4:
  %29 = load i16, ptr %7, !tbaa !2
  %30 = add i16 %29, 1
  store i16 %30, ptr %7, !tbaa !2
  br label %b2

b5:
  store i16 0, ptr %5, !tbaa !2
  store i8 -1, ptr %4, !tbaa !2
  %31 = load i16, ptr addrspace(1) %17
  store i16 0, ptr %3, !tbaa !2
  br label %b6

b6:
  %32 = load i16, ptr %3, !tbaa !2
  %33 = icmp ult i16 %32, %31
  %34 = sext i1 %33 to i8
  %35 = icmp ne i8 %34, 0
  br i1 %35, label %b7, label %b9

b7:
  %36 = getelementptr i8, ptr addrspace(1) %17, i16 4
  %37 = load ptr addrspace(1), ptr addrspace(1) %36, !tbaa !2
  %38 = getelementptr i8, ptr addrspace(1) %37, i16 %32
  %39 = load i8, ptr addrspace(1) %38
  %40 = icmp eq i8 %39, 32
  %41 = sext i1 %40 to i8
  %42 = icmp ne i8 %41, 0
  br i1 %42, label %b10, label %b11

b8:
  %43 = load i16, ptr %3, !tbaa !2
  %44 = add i16 %43, 1
  store i16 %44, ptr %3, !tbaa !2
  br label %b6

b9:
  %45 = addrspacecast ptr %10 to ptr addrspace(1)
  store i16 2, ptr %1, !tbaa !2
  %46 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 2, ptr %46, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %45, ptr %47, !tbaa !2
  %48 = addrspacecast ptr %1 to ptr addrspace(1)
  %49 = load i16, ptr addrspace(1) %48, !tbaa !2
  store i16 %49, ptr addrspace(1) %14, !tbaa !2
  %50 = getelementptr i8, ptr addrspace(1) %48, i16 2
  %51 = load i16, ptr addrspace(1) %50, !tbaa !2
  %52 = getelementptr i8, ptr addrspace(1) %14, i16 2
  store i16 %51, ptr addrspace(1) %52, !tbaa !2
  %53 = getelementptr i8, ptr addrspace(1) %48, i16 4
  %54 = load ptr addrspace(1), ptr addrspace(1) %53, !tbaa !2
  %55 = getelementptr i8, ptr addrspace(1) %14, i16 4
  store ptr addrspace(1) %54, ptr addrspace(1) %55, !tbaa !2
  %56 = getelementptr i8, ptr addrspace(1) %14, i16 4
  %57 = load ptr addrspace(1), ptr addrspace(1) %56, !tbaa !2
  %58 = load i16, ptr addrspace(1) %14, !tbaa !2
  %59 = call addrspace(1) ptr @abi.qb45.string_result(ptr addrspace(1) %57, i16 %58)
  ret ptr %59

b10:
  store i8 -1, ptr %4, !tbaa !2
  br label %b12

b11:
  %60 = load i8, ptr %4, !tbaa !2
  store i8 %60, ptr %2, !tbaa !2
  %61 = icmp ne i8 %60, 0
  br i1 %61, label %b13, label %b14

b12:
  br label %b8

b13:
  %62 = load i16, ptr %5, !tbaa !2
  %63 = icmp ult i16 %62, 2
  %64 = sext i1 %63 to i8
  store i8 %64, ptr %2, !tbaa !2
  br label %b14

b14:
  %65 = load i8, ptr %2, !tbaa !2
  %66 = icmp ne i8 %65, 0
  br i1 %66, label %b15, label %b16

b15:
  %67 = load i16, ptr %5, !tbaa !2
  %68 = load i8, ptr addrspace(1) %38
  %69 = sub i16 %67, 0
  %70 = getelementptr inbounds i8, ptr %10, i16 %69
  store i8 %68, ptr %70, !tbaa !2
  %71 = load i16, ptr %5, !tbaa !2
  %72 = add i16 %71, 1
  store i16 %72, ptr %5, !tbaa !2
  br label %b17

b16:
  br label %b17

b17:
  store i8 0, ptr %4, !tbaa !2
  br label %b12
}

declare cc1000 ptr @B$SCPY(ptr) addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare cc1000 double @MEAN(ptr, ptr, ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
