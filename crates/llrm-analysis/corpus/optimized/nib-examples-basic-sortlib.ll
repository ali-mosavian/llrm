target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$var_abi.qb45.result = internal global [4 x i8] zeroinitializer

define internal ptr addrspace(1) @abi.basic.array_data(ptr %0) addrspace(1) memory(argmem: read) willreturn {
b1:
  %1 = load ptr addrspace(1), ptr %0
  ret ptr addrspace(1) %1
}

define internal i16 @abi.basic.array_count(ptr %0, i16 %1) addrspace(1) memory(argmem: read) willreturn {
b1:
  %2 = getelementptr i8, ptr %0, i16 14
  %3 = shl i16 %1, 1
  %4 = shl i16 %3, 1
  %5 = getelementptr i8, ptr %2, i16 %4
  %6 = load i16, ptr %5
  ret i16 %6
}

define internal ptr addrspace(1) @abi.qb45.string_data(ptr %0) addrspace(1) memory(argmem: read) willreturn {
b1:
  %1 = getelementptr i8, ptr %0, i16 2
  %2 = load ptr, ptr %1
  %3 = addrspacecast ptr %2 to ptr addrspace(1)
  ret ptr addrspace(1) %3
}

define internal i16 @abi.qb45.string_length(ptr %0) addrspace(1) memory(argmem: read) willreturn {
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
  %2 = load ptr addrspace(1), ptr %0
  %3 = load i16, ptr %1
  br label %b2

b2:
  %4 = phi i16 [ 1, %b1 ], [ %20, %b7 ]
  %5 = icmp ult i16 %4, %3
  br i1 %5, label %b3, label %b4

b3:
  %6 = shl i16 %4, 1
  %7 = getelementptr i8, ptr addrspace(1) %2, i16 %6
  %8 = load i16, ptr addrspace(1) %7
  br label %b5

b4:
  ret void

b5:
  %9 = phi i16 [ %4, %b3 ], [ %14, %b6 ]
  %10 = icmp ugt i16 %9, 0
  %11 = sext i1 %10 to i8
  br i1 %10, label %b8, label %b9

b6:
  %12 = shl i16 %9, 1
  %13 = getelementptr i8, ptr addrspace(1) %2, i16 %12
  %14 = add i16 %9, -1
  %15 = shl i16 %14, 1
  %16 = getelementptr i8, ptr addrspace(1) %2, i16 %15
  %17 = load i16, ptr addrspace(1) %16
  store i16 %17, ptr addrspace(1) %13
  br label %b5

b7:
  %18 = shl i16 %9, 1
  %19 = getelementptr i8, ptr addrspace(1) %2, i16 %18
  store i16 %8, ptr addrspace(1) %19
  %20 = add i16 %4, 1
  br label %b2

b8:
  %21 = add i16 %9, -1
  %22 = shl i16 %21, 1
  %23 = getelementptr i8, ptr addrspace(1) %2, i16 %22
  %24 = load i16, ptr addrspace(1) %23
  %25 = icmp slt i16 %24, %8
  %26 = sext i1 %25 to i8
  br label %b9

b9:
  %27 = phi i8 [ %11, %b5 ], [ %26, %b8 ]
  %28 = icmp ne i8 %27, 0
  br i1 %28, label %b6, label %b7
}

define cc1000 void @UPPER(ptr %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr %0, i16 2
  %2 = load ptr, ptr %1
  %3 = addrspacecast ptr %2 to ptr addrspace(1)
  %4 = load i16, ptr %0
  br label %b2

b2:
  %5 = phi i16 [ 0, %b1 ], [ %18, %b9 ]
  %6 = icmp ult i16 %5, %4
  br i1 %6, label %b3, label %b4

b3:
  %7 = getelementptr i8, ptr addrspace(1) %3, i16 %5
  %8 = load i8, ptr addrspace(1) %7
  %9 = icmp uge i8 %8, 97
  %10 = sext i1 %9 to i8
  br i1 %9, label %b6, label %b5

b4:
  ret void

b5:
  %11 = phi i8 [ %10, %b3 ], [ %14, %b6 ]
  %12 = icmp ne i8 %11, 0
  br i1 %12, label %b7, label %b9

b6:
  %13 = icmp ule i8 %8, 122
  %14 = sext i1 %13 to i8
  br label %b5

b7:
  %15 = zext i8 %8 to i16
  %16 = add i16 %15, -32
  %17 = trunc i16 %16 to i8
  store i8 %17, ptr addrspace(1) %7
  br label %b9

b9:
  %18 = add i16 %5, 1
  br label %b2
}

define cc1000 double @AVERAGE(ptr %0, ptr %1, ptr %2) addrspace(1) {
b1:
  %3 = alloca double
  %4 = alloca i16
  %5 = alloca i16
  store double 0.000000e+00, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  %6 = load ptr addrspace(1), ptr %0
  store i16 0, ptr %5, !tbaa !2
  %7 = load i16, ptr %1
  store i16 %7, ptr %4, !tbaa !2
  br label %b2

b2:
  %8 = phi i16 [ 0, %b1 ], [ %15, %b3 ]
  %9 = icmp ult i16 %8, %7
  br i1 %9, label %b3, label %b4

b3:
  %10 = load i16, ptr %5, !tbaa !2
  %11 = shl i16 %8, 1
  %12 = getelementptr i8, ptr addrspace(1) %6, i16 %11
  %13 = load i16, ptr addrspace(1) %12
  %14 = add i16 %10, %13
  store i16 %14, ptr %5, !tbaa !2
  %15 = add i16 %8, 1
  br label %b2

b4:
  %16 = call cc1000 addrspace(1) double @MEAN(ptr %5, ptr %4, ptr %3)
  ret double %16
}

define cc1000 i16 @ROWTOTAL(ptr %0, ptr %1) addrspace(1) {
b1:
  %2 = load ptr addrspace(1), ptr %0
  %3 = getelementptr i8, ptr %0, i16 14
  %4 = getelementptr i8, ptr %3, i16 0
  %5 = load i16, ptr %4
  %6 = getelementptr i8, ptr %3, i16 4
  %7 = load i16, ptr %6
  br label %b2

b2:
  %8 = phi i16 [ 0, %b1 ], [ %17, %b3 ]
  %9 = phi i16 [ 0, %b1 ], [ %18, %b3 ]
  %10 = icmp ult i16 %9, %5
  br i1 %10, label %b3, label %b4

b3:
  %11 = load i16, ptr %1
  %12 = mul i16 %9, %7
  %13 = add i16 %12, %11
  %14 = shl i16 %13, 1
  %15 = getelementptr i8, ptr addrspace(1) %2, i16 %14
  %16 = load i16, ptr addrspace(1) %15
  %17 = add i16 %8, %16
  %18 = add i16 %9, 1
  br label %b2

b4:
  ret i16 %8
}

define cc1000 ptr @INITIALS(ptr %0) addrspace(1) {
b1:
  %1 = alloca ptr addrspace(1)
  %2 = alloca [2 x i8]
  %3 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 2, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  %4 = getelementptr i8, ptr %0, i16 2
  %5 = load ptr, ptr %4
  %6 = addrspacecast ptr %5 to ptr addrspace(1)
  %7 = load i16, ptr %0
  %8 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 %7, ptr addrspace(1) %8, !tbaa !2
  %9 = getelementptr i8, ptr addrspace(1) %8, i16 2
  store i16 %7, ptr addrspace(1) %9, !tbaa !2
  %10 = getelementptr i8, ptr addrspace(1) %8, i16 4
  store ptr addrspace(1) %6, ptr addrspace(1) %10, !tbaa !2
  br label %b2

b2:
  %11 = phi i16 [ 0, %b1 ], [ %14, %b3 ]
  %12 = icmp slt i16 %11, 2
  br i1 %12, label %b3, label %b5

b3:
  %13 = getelementptr inbounds i8, ptr %2, i16 %11
  store i8 63, ptr %13, !tbaa !2
  %14 = add i16 %11, 1
  br label %b2

b5:
  %15 = load i16, ptr addrspace(1) %8
  %16 = load ptr addrspace(1), ptr addrspace(1) %10, !tbaa !2
  br label %b6

b6:
  %17 = phi i16 [ 0, %b5 ], [ %29, %b12 ]
  %18 = phi i8 [ -1, %b5 ], [ %30, %b12 ]
  %19 = phi i16 [ 0, %b5 ], [ %31, %b12 ]
  %20 = icmp ult i16 %19, %15
  br i1 %20, label %b7, label %b9

b7:
  %21 = getelementptr i8, ptr addrspace(1) %16, i16 %19
  %22 = load i8, ptr addrspace(1) %21
  %23 = icmp eq i8 %22, 32
  br i1 %23, label %b12, label %b11

b9:
  %24 = addrspacecast ptr %2 to ptr addrspace(1)
  store ptr addrspace(1) null, ptr %1
  store i16 2, ptr @$var_abi.qb45.result
  store ptr addrspace(1) %24, ptr %1
  %25 = load ptr, ptr %1
  %26 = getelementptr inbounds i8, ptr @$var_abi.qb45.result, i16 2
  store ptr %25, ptr %26
  %27 = call cc1000 addrspace(1) ptr @B$SCPY(ptr @$var_abi.qb45.result)
  ret ptr %27

b11:
  %28 = icmp ne i8 %18, 0
  br i1 %28, label %b13, label %b14

b12:
  %29 = phi i16 [ %17, %b7 ], [ %39, %b17 ]
  %30 = phi i8 [ -1, %b7 ], [ 0, %b17 ]
  %31 = add i16 %19, 1
  br label %b6

b13:
  %32 = icmp ult i16 %17, 2
  %33 = sext i1 %32 to i8
  br label %b14

b14:
  %34 = phi i8 [ %18, %b11 ], [ %33, %b13 ]
  %35 = icmp ne i8 %34, 0
  br i1 %35, label %b15, label %b17

b15:
  %36 = load i8, ptr addrspace(1) %21
  %37 = getelementptr inbounds i8, ptr %2, i16 %17
  store i8 %36, ptr %37, !tbaa !2
  %38 = add i16 %17, 1
  br label %b17

b17:
  %39 = phi i16 [ %38, %b15 ], [ %17, %b14 ]
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
