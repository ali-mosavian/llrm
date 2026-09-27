target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [20 x i8] c"\08\00\0D\00\0D\00sum_three: ok\00"
@$str2 = internal constant [21 x i8] c"\08\00\0E\00\0E\00sum_three: bad\00"

define internal i16 @sum_three(ptr addrspace(1) noalias readonly dereferenceable(8) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1, ptr addrspace(1) noalias readonly dereferenceable(8) %2) addrspace(1) {
b1:
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %5, !tbaa !2
  %6 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %4, !tbaa !2
  store i16 %6, ptr %3, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %4, !tbaa !2
  %8 = load i16, ptr %3, !tbaa !2
  %9 = icmp ult i16 %7, %8
  %10 = sext i1 %9 to i8
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b3, label %b5

b3:
  %12 = load i16, ptr %5, !tbaa !2
  %13 = load i16, ptr %4, !tbaa !2
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %15 = load ptr addrspace(1), ptr addrspace(1) %14
  %16 = mul i16 %13, 2
  %17 = getelementptr i8, ptr addrspace(1) %15, i16 %16
  %18 = load i16, ptr addrspace(1) %17
  %19 = add i16 %12, %18
  store i16 %19, ptr %5, !tbaa !2
  %20 = load i16, ptr %5, !tbaa !2
  %21 = load i16, ptr %4, !tbaa !2
  %22 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %23 = load ptr addrspace(1), ptr addrspace(1) %22
  %24 = mul i16 %21, 2
  %25 = getelementptr i8, ptr addrspace(1) %23, i16 %24
  %26 = load i16, ptr addrspace(1) %25
  %27 = add i16 %20, %26
  store i16 %27, ptr %5, !tbaa !2
  %28 = load i16, ptr %5, !tbaa !2
  %29 = load i16, ptr %4, !tbaa !2
  %30 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %31 = load ptr addrspace(1), ptr addrspace(1) %30
  %32 = mul i16 %29, 2
  %33 = getelementptr i8, ptr addrspace(1) %31, i16 %32
  %34 = load i16, ptr addrspace(1) %33
  %35 = add i16 %28, %34
  store i16 %35, ptr %5, !tbaa !2
  br label %b4

b4:
  %36 = load i16, ptr %4, !tbaa !2
  %37 = add i16 %36, 1
  store i16 %37, ptr %4, !tbaa !2
  br label %b2

b5:
  %38 = load i16, ptr %5, !tbaa !2
  ret i16 %38
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca [8 x i8]
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca [8 x i8]
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca [8 x i8]
  store i16 0, ptr %0
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  store i16 0, ptr %4
  store i16 0, ptr %5
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 8, i1 false)
  store i16 0, ptr %7
  store i16 0, ptr %8
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 8, i1 false)
  store i16 0, ptr %10
  store i16 0, ptr %11
  call void @llvm.memset.p0.i16(ptr %12, i8 0, i16 8, i1 false)
  store i16 4, ptr %10, !tbaa !2
  store i16 4, ptr %11, !tbaa !2
  %13 = sub i16 0, 0
  %14 = getelementptr inbounds i16, ptr %12, i16 %13
  store i16 1, ptr %14, !tbaa !2
  %15 = sub i16 1, 0
  %16 = getelementptr inbounds i16, ptr %12, i16 %15
  store i16 2, ptr %16, !tbaa !2
  %17 = sub i16 2, 0
  %18 = getelementptr inbounds i16, ptr %12, i16 %17
  store i16 3, ptr %18, !tbaa !2
  %19 = sub i16 3, 0
  %20 = getelementptr inbounds i16, ptr %12, i16 %19
  store i16 4, ptr %20, !tbaa !2
  store i16 4, ptr %7, !tbaa !2
  store i16 4, ptr %8, !tbaa !2
  %21 = sub i16 0, 0
  %22 = getelementptr inbounds i16, ptr %9, i16 %21
  store i16 10, ptr %22, !tbaa !2
  %23 = sub i16 1, 0
  %24 = getelementptr inbounds i16, ptr %9, i16 %23
  store i16 20, ptr %24, !tbaa !2
  %25 = sub i16 2, 0
  %26 = getelementptr inbounds i16, ptr %9, i16 %25
  store i16 30, ptr %26, !tbaa !2
  %27 = sub i16 3, 0
  %28 = getelementptr inbounds i16, ptr %9, i16 %27
  store i16 40, ptr %28, !tbaa !2
  store i16 4, ptr %4, !tbaa !2
  store i16 4, ptr %5, !tbaa !2
  %29 = sub i16 0, 0
  %30 = getelementptr inbounds i16, ptr %6, i16 %29
  store i16 100, ptr %30, !tbaa !2
  %31 = sub i16 1, 0
  %32 = getelementptr inbounds i16, ptr %6, i16 %31
  store i16 200, ptr %32, !tbaa !2
  %33 = sub i16 2, 0
  %34 = getelementptr inbounds i16, ptr %6, i16 %33
  store i16 300, ptr %34, !tbaa !2
  %35 = sub i16 3, 0
  %36 = getelementptr inbounds i16, ptr %6, i16 %35
  store i16 400, ptr %36, !tbaa !2
  %37 = addrspacecast ptr %12 to ptr addrspace(1)
  store i16 4, ptr %3, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 4, ptr %38, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %37, ptr %39, !tbaa !2
  %40 = addrspacecast ptr %3 to ptr addrspace(1)
  %41 = addrspacecast ptr %9 to ptr addrspace(1)
  store i16 4, ptr %2, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 4, ptr %42, !tbaa !2
  %43 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %41, ptr %43, !tbaa !2
  %44 = addrspacecast ptr %2 to ptr addrspace(1)
  %45 = addrspacecast ptr %6 to ptr addrspace(1)
  store i16 4, ptr %1, !tbaa !2
  %46 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 4, ptr %46, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %45, ptr %47, !tbaa !2
  %48 = addrspacecast ptr %1 to ptr addrspace(1)
  %49 = call addrspace(1) i16 @sum_three(ptr addrspace(1) %40, ptr addrspace(1) %44, ptr addrspace(1) %48)
  store i16 %49, ptr %0, !tbaa !2
  %50 = load i16, ptr %0, !tbaa !2
  %51 = icmp eq i16 %50, 1110
  %52 = sext i1 %51 to i8
  %53 = icmp ne i8 %52, 0
  br i1 %53, label %b2, label %b3

b2:
  %54 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %54)
  call addrspace(1) void @N$PN()
  br label %b4

b3:
  %55 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %55)
  call addrspace(1) void @N$PN()
  br label %b4

b4:
  %56 = load i16, ptr %0, !tbaa !2
  ret i16 %56
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
