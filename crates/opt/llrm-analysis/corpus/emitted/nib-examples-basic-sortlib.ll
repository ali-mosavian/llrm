target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$var_abi.qb45.result = internal global [4 x i8] zeroinitializer

define internal cc 1005 ptr addrspace(1) @abi.basic.array_data(ptr %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = load ptr addrspace(1), ptr %0
  ret ptr addrspace(1) %1
}

define internal cc 1005 i16 @abi.basic.array_count(ptr %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = getelementptr i8, ptr %0, i16 14
  %3 = mul i16 2, %1
  %4 = mul i16 %3, 2
  %5 = getelementptr i8, ptr %2, i16 %4
  %6 = load i16, ptr %5
  ret i16 %6
}

define internal cc 1005 ptr addrspace(1) @abi.qb45.string_data(ptr %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca ptr
  %2 = getelementptr i8, ptr %0, i16 2
  %3 = load ptr, ptr %2
  store ptr %3, ptr %1, !tbaa !2
  %4 = load ptr, ptr %1, !tbaa !2
  %5 = addrspacecast ptr %4 to ptr addrspace(1)
  ret ptr addrspace(1) %5
}

define internal cc 1005 i16 @abi.qb45.string_length(ptr %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = load i16, ptr %0
  ret i16 %1
}

define internal cc 1005 ptr @abi.qb45.string_result(ptr addrspace(1) %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca ptr addrspace(1)
  store i16 %1, ptr @$var_abi.qb45.result, !tbaa !2
  store ptr addrspace(1) %0, ptr %2, !tbaa !2
  %3 = load ptr, ptr %2, !tbaa !2
  %4 = getelementptr inbounds i8, ptr @$var_abi.qb45.result, i16 2
  store ptr %3, ptr %4, !tbaa !2
  %5 = call cc 1000 addrspace(1) ptr @B$SCPY(ptr @$var_abi.qb45.result)
  ret ptr %5
}

define cc 1000 void @SORTSCORES(ptr %0, ptr %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca [8 x i8]
  %7 = call cc 1005 addrspace(1) ptr addrspace(1) @abi.basic.array_data(ptr %0)
  %8 = call cc 1005 addrspace(1) i16 @abi.basic.array_count(ptr %0, i16 0)
  %9 = addrspacecast ptr %6 to ptr addrspace(1)
  store i16 %8, ptr addrspace(1) %9, !tbaa !2
  %10 = getelementptr i8, ptr addrspace(1) %9, i16 2
  store i16 %8, ptr addrspace(1) %10, !tbaa !2
  %11 = getelementptr i8, ptr addrspace(1) %9, i16 4
  store ptr addrspace(1) %7, ptr addrspace(1) %11, !tbaa !2
  %12 = load i16, ptr %1
  store i16 %12, ptr %5, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  br label %b2

b2:
  %13 = load i16, ptr %4, !tbaa !2
  %14 = load i16, ptr %5, !tbaa !2
  %15 = icmp ult i16 %13, %14
  br i1 %15, label %b3, label %b4

b3:
  %16 = load i16, ptr %4, !tbaa !2
  %17 = getelementptr i8, ptr addrspace(1) %9, i16 4
  %18 = load ptr addrspace(1), ptr addrspace(1) %17, !tbaa !2
  %19 = mul i16 %16, 2
  %20 = getelementptr inbounds i8, ptr addrspace(1) %18, i16 %19
  %21 = load i16, ptr addrspace(1) %20
  store i16 %21, ptr %3, !tbaa !2
  %22 = load i16, ptr %4, !tbaa !2
  store i16 %22, ptr %2, !tbaa !2
  br label %b5

b4:
  ret void

b5:
  %23 = load i16, ptr %2, !tbaa !2
  %24 = icmp ugt i16 %23, 0
  br i1 %24, label %b8, label %b7

b6:
  %25 = load i16, ptr %2, !tbaa !2
  %26 = getelementptr i8, ptr addrspace(1) %9, i16 4
  %27 = load ptr addrspace(1), ptr addrspace(1) %26, !tbaa !2
  %28 = mul i16 %25, 2
  %29 = getelementptr inbounds i8, ptr addrspace(1) %27, i16 %28
  %30 = load i16, ptr %2, !tbaa !2
  %31 = sub i16 %30, 1
  %32 = getelementptr i8, ptr addrspace(1) %9, i16 4
  %33 = load ptr addrspace(1), ptr addrspace(1) %32, !tbaa !2
  %34 = mul i16 %31, 2
  %35 = getelementptr inbounds i8, ptr addrspace(1) %33, i16 %34
  %36 = load i16, ptr addrspace(1) %35
  store i16 %36, ptr addrspace(1) %29
  %37 = load i16, ptr %2, !tbaa !2
  %38 = sub i16 %37, 1
  store i16 %38, ptr %2, !tbaa !2
  br label %b5

b7:
  %39 = load i16, ptr %2, !tbaa !2
  %40 = getelementptr i8, ptr addrspace(1) %9, i16 4
  %41 = load ptr addrspace(1), ptr addrspace(1) %40, !tbaa !2
  %42 = mul i16 %39, 2
  %43 = getelementptr inbounds i8, ptr addrspace(1) %41, i16 %42
  %44 = load i16, ptr %3, !tbaa !2
  store i16 %44, ptr addrspace(1) %43
  %45 = load i16, ptr %4, !tbaa !2
  %46 = add i16 %45, 1
  store i16 %46, ptr %4, !tbaa !2
  br label %b2

b8:
  %47 = load i16, ptr %2, !tbaa !2
  %48 = sub i16 %47, 1
  %49 = getelementptr i8, ptr addrspace(1) %9, i16 4
  %50 = load ptr addrspace(1), ptr addrspace(1) %49, !tbaa !2
  %51 = mul i16 %48, 2
  %52 = getelementptr inbounds i8, ptr addrspace(1) %50, i16 %51
  %53 = load i16, ptr addrspace(1) %52
  %54 = load i16, ptr %3, !tbaa !2
  %55 = icmp slt i16 %53, %54
  br i1 %55, label %b6, label %b7
}

define cc 1000 void @UPPER(ptr %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i8
  %2 = alloca i8
  %3 = alloca i16
  %4 = alloca [8 x i8]
  %5 = call cc 1005 addrspace(1) ptr addrspace(1) @abi.qb45.string_data(ptr %0)
  %6 = call cc 1005 addrspace(1) i16 @abi.qb45.string_length(ptr %0)
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
  br i1 %12, label %b3, label %b4

b3:
  %13 = load i16, ptr %3, !tbaa !2
  %14 = getelementptr i8, ptr addrspace(1) %7, i16 4
  %15 = load ptr addrspace(1), ptr addrspace(1) %14, !tbaa !2
  %16 = getelementptr inbounds i8, ptr addrspace(1) %15, i16 %13
  %17 = load i8, ptr addrspace(1) %16
  store i8 %17, ptr %2, !tbaa !2
  %18 = load i8, ptr %2, !tbaa !2
  %19 = icmp ule i8 97, %18
  %20 = zext i1 %19 to i8
  store i8 %20, ptr %1, !tbaa !2
  br i1 %19, label %b9, label %b8

b4:
  ret void

b5:
  %21 = load i16, ptr %3, !tbaa !2
  %22 = getelementptr i8, ptr addrspace(1) %7, i16 4
  %23 = load ptr addrspace(1), ptr addrspace(1) %22, !tbaa !2
  %24 = getelementptr inbounds i8, ptr addrspace(1) %23, i16 %21
  %25 = load i8, ptr %2, !tbaa !2
  %26 = zext i8 %25 to i16
  %27 = sub i16 %26, 32
  %28 = trunc i16 %27 to i8
  store i8 %28, ptr addrspace(1) %24
  br label %b7

b6:
  br label %b7

b7:
  %29 = load i16, ptr %3, !tbaa !2
  %30 = add i16 %29, 1
  store i16 %30, ptr %3, !tbaa !2
  br label %b2

b8:
  %31 = load i8, ptr %1, !tbaa !2, !range !5
  %32 = icmp ne i8 %31, 0
  br i1 %32, label %b5, label %b6

b9:
  %33 = icmp ule i8 %18, 122
  %34 = zext i1 %33 to i8
  store i8 %34, ptr %1, !tbaa !2
  br label %b8
}

define cc 1000 ptr @AVERAGE(ptr %0, ptr %1, ptr %2) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca double
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca [8 x i8]
  %8 = call cc 1005 addrspace(1) ptr addrspace(1) @abi.basic.array_data(ptr %0)
  %9 = call cc 1005 addrspace(1) i16 @abi.basic.array_count(ptr %0, i16 0)
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
  br i1 %16, label %b3, label %b4

b3:
  %17 = load i16, ptr %6, !tbaa !2
  %18 = load i16, ptr %4, !tbaa !2
  %19 = getelementptr i8, ptr addrspace(1) %10, i16 4
  %20 = load ptr addrspace(1), ptr addrspace(1) %19, !tbaa !2
  %21 = mul i16 %18, 2
  %22 = getelementptr inbounds i8, ptr addrspace(1) %20, i16 %21
  %23 = load i16, ptr addrspace(1) %22
  %24 = add i16 %17, %23
  store i16 %24, ptr %6, !tbaa !2
  %25 = load i16, ptr %4, !tbaa !2
  %26 = add i16 %25, 1
  store i16 %26, ptr %4, !tbaa !2
  br label %b2

b4:
  %27 = call cc 1000 addrspace(1) ptr @MEAN(ptr %6, ptr %5, ptr %3)
  %28 = load double, ptr %27
  store double %28, ptr %2
  ret ptr %2
}

define cc 1000 i16 @ROWTOTAL(ptr %0, ptr %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca [10 x i8]
  %5 = call cc 1005 addrspace(1) ptr addrspace(1) @abi.basic.array_data(ptr %0)
  %6 = call cc 1005 addrspace(1) i16 @abi.basic.array_count(ptr %0, i16 0)
  %7 = call cc 1005 addrspace(1) i16 @abi.basic.array_count(ptr %0, i16 1)
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
  br i1 %15, label %b3, label %b4

b3:
  %16 = load i16, ptr %3, !tbaa !2
  %17 = load i16, ptr %2, !tbaa !2
  %18 = load i16, ptr %1
  %19 = getelementptr i8, ptr addrspace(1) %8, i16 2
  %20 = load i16, ptr addrspace(1) %19, !tbaa !2
  %21 = mul i16 %20, 1
  %22 = mul i16 %17, %21
  %23 = mul i16 %18, 1
  %24 = add i16 %22, %23
  %25 = getelementptr i8, ptr addrspace(1) %8, i16 6
  %26 = load ptr addrspace(1), ptr addrspace(1) %25, !tbaa !2
  %27 = mul i16 %24, 2
  %28 = getelementptr inbounds i8, ptr addrspace(1) %26, i16 %27
  %29 = load i16, ptr addrspace(1) %28
  %30 = add i16 %16, %29
  store i16 %30, ptr %3, !tbaa !2
  %31 = load i16, ptr %2, !tbaa !2
  %32 = add i16 %31, 1
  store i16 %32, ptr %2, !tbaa !2
  br label %b2

b4:
  %33 = load i16, ptr %3, !tbaa !2
  ret i16 %33
}

define cc 1000 ptr @INITIALS(ptr %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca i16
  %3 = alloca i8
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca [2 x i8]
  %10 = alloca i8
  %11 = alloca [8 x i8]
  %12 = alloca [8 x i8]
  %13 = addrspacecast ptr %12 to ptr addrspace(1)
  %14 = call cc 1005 addrspace(1) ptr addrspace(1) @abi.qb45.string_data(ptr %0)
  %15 = call cc 1005 addrspace(1) i16 @abi.qb45.string_length(ptr %0)
  %16 = addrspacecast ptr %11 to ptr addrspace(1)
  store i16 %15, ptr addrspace(1) %16, !tbaa !2
  %17 = getelementptr i8, ptr addrspace(1) %16, i16 2
  store i16 %15, ptr addrspace(1) %17, !tbaa !2
  %18 = getelementptr i8, ptr addrspace(1) %16, i16 4
  store ptr addrspace(1) %14, ptr addrspace(1) %18, !tbaa !2
  store i8 63, ptr %10, !tbaa !2
  store i16 2, ptr %7, !tbaa !2
  store i16 2, ptr %8, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  store i16 2, ptr %5, !tbaa !2
  br label %b2

b2:
  %19 = load i16, ptr %6, !tbaa !2
  %20 = load i16, ptr %5, !tbaa !2
  %21 = icmp slt i16 %19, %20
  br i1 %21, label %b3, label %b5

b3:
  %22 = load i16, ptr %6, !tbaa !2
  %23 = load i8, ptr %10, !tbaa !2
  %24 = sub i16 %22, 0
  %25 = getelementptr inbounds i8, ptr %9, i16 %24
  store i8 %23, ptr %25, !tbaa !2
  br label %b4

b4:
  %26 = load i16, ptr %6, !tbaa !2
  %27 = add nsw i16 %26, 1
  store i16 %27, ptr %6, !tbaa !2
  br label %b2

b5:
  store i16 0, ptr %4, !tbaa !2
  store i8 1, ptr %3, !tbaa !2
  %28 = load i16, ptr addrspace(1) %16
  store i16 0, ptr %2, !tbaa !2
  br label %b6

b6:
  %29 = load i16, ptr %2, !tbaa !2
  %30 = icmp ult i16 %29, %28
  br i1 %30, label %b7, label %b9

b7:
  %31 = getelementptr i8, ptr addrspace(1) %16, i16 4
  %32 = load ptr addrspace(1), ptr addrspace(1) %31, !tbaa !2
  %33 = getelementptr inbounds i8, ptr addrspace(1) %32, i16 %29
  %34 = load i8, ptr addrspace(1) %33
  %35 = icmp eq i8 %34, 32
  br i1 %35, label %b10, label %b11

b8:
  %36 = load i16, ptr %2, !tbaa !2
  %37 = add i16 %36, 1
  store i16 %37, ptr %2, !tbaa !2
  br label %b6

b9:
  %38 = addrspacecast ptr %9 to ptr addrspace(1)
  store i16 2, ptr %1, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 2, ptr %39, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %38, ptr %40, !tbaa !2
  %41 = addrspacecast ptr %1 to ptr addrspace(1)
  %42 = load i16, ptr addrspace(1) %41, !tbaa !2
  store i16 %42, ptr addrspace(1) %13, !tbaa !2
  %43 = getelementptr i8, ptr addrspace(1) %41, i16 2
  %44 = load i16, ptr addrspace(1) %43, !tbaa !2
  %45 = getelementptr i8, ptr addrspace(1) %13, i16 2
  store i16 %44, ptr addrspace(1) %45, !tbaa !2
  %46 = getelementptr i8, ptr addrspace(1) %41, i16 4
  %47 = load ptr addrspace(1), ptr addrspace(1) %46, !tbaa !2
  %48 = getelementptr i8, ptr addrspace(1) %13, i16 4
  store ptr addrspace(1) %47, ptr addrspace(1) %48, !tbaa !2
  %49 = getelementptr i8, ptr addrspace(1) %13, i16 4
  %50 = load ptr addrspace(1), ptr addrspace(1) %49, !tbaa !2
  %51 = load i16, ptr addrspace(1) %13, !tbaa !2
  %52 = call cc 1005 addrspace(1) ptr @abi.qb45.string_result(ptr addrspace(1) %50, i16 %51)
  ret ptr %52

b10:
  store i8 1, ptr %3, !tbaa !2
  br label %b12

b11:
  %53 = load i8, ptr %3, !tbaa !2, !range !6
  %54 = icmp ne i8 %53, 0
  br i1 %54, label %b16, label %b14

b12:
  br label %b8

b13:
  %55 = load i16, ptr %4, !tbaa !2
  %56 = load i8, ptr addrspace(1) %33
  %57 = sub i16 %55, 0
  %58 = getelementptr inbounds i8, ptr %9, i16 %57
  store i8 %56, ptr %58, !tbaa !2
  %59 = load i16, ptr %4, !tbaa !2
  %60 = add i16 %59, 1
  store i16 %60, ptr %4, !tbaa !2
  br label %b15

b14:
  br label %b15

b15:
  store i8 0, ptr %3, !tbaa !2
  br label %b12

b16:
  %61 = load i16, ptr %4, !tbaa !2
  %62 = icmp ult i16 %61, 2
  br i1 %62, label %b13, label %b14
}

declare cc 1000 ptr @B$SCPY(ptr) addrspace(1)

declare cc 1000 ptr @MEAN(ptr, ptr, ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
