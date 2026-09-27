target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @at(ptr addrspace(1) noalias readonly dereferenceable(10) %0, i8 %1) addrspace(1) {
b1:
  %2 = load i16, ptr addrspace(1) %0
  %3 = zext i8 %1 to i16
  %4 = icmp ult i16 %3, %2
  %5 = sext i1 %4 to i8
  %6 = icmp ne i8 %5, 0
  br i1 %6, label %b2, label %b3

b2:
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %8 = load i16, ptr addrspace(1) %7
  %9 = icmp ult i16 1, %8
  %10 = sext i1 %9 to i8
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b4, label %b5

b3:
  call addrspace(1) void @N$EBND()
  unreachable

b4:
  %12 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %13 = load i16, ptr addrspace(1) %12
  %14 = mul i16 %13, 1
  %15 = zext i8 %1 to i16
  %16 = mul i16 %15, %14
  %17 = add i16 %16, 1
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %19 = load ptr addrspace(1), ptr addrspace(1) %18
  %20 = mul i16 %17, 2
  %21 = getelementptr i8, ptr addrspace(1) %19, i16 %20
  %22 = load i16, ptr addrspace(1) %21
  ret i16 %22

b5:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @value() addrspace(1) {
b1:
  %0 = alloca [10 x i8]
  %1 = alloca i8
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca [800 x i8]
  %10 = alloca i16
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 10, i1 false)
  store i8 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 800, i1 false)
  store i16 0, ptr %10
  store i16 0, ptr %10, !tbaa !2
  store i16 20, ptr %6, !tbaa !2
  store i16 20, ptr %7, !tbaa !2
  store i16 400, ptr %8, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  store i16 20, ptr %4, !tbaa !2
  br label %b2

b2:
  %11 = load i16, ptr %5, !tbaa !2
  %12 = load i16, ptr %4, !tbaa !2
  %13 = icmp slt i16 %11, %12
  %14 = sext i1 %13 to i8
  %15 = icmp ne i8 %14, 0
  br i1 %15, label %b3, label %b5

b3:
  store i16 0, ptr %3, !tbaa !2
  store i16 20, ptr %2, !tbaa !2
  br label %b6

b4:
  %16 = load i16, ptr %5, !tbaa !2
  %17 = add i16 %16, 1
  store i16 %17, ptr %5, !tbaa !2
  br label %b2

b5:
  %18 = sub i16 19, 0
  %19 = sub i16 1, 0
  %20 = mul i16 %18, 20
  %21 = add i16 %20, %19
  %22 = getelementptr inbounds i16, ptr %9, i16 %21
  store i16 7, ptr %22, !tbaa !2
  store i8 19, ptr %1, !tbaa !2
  %23 = addrspacecast ptr %9 to ptr addrspace(1)
  store i16 20, ptr %0, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 20, ptr %24, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %0, i16 4
  store i16 400, ptr %25, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %0, i16 6
  store ptr addrspace(1) %23, ptr %26, !tbaa !2
  %27 = addrspacecast ptr %0 to ptr addrspace(1)
  %28 = load i8, ptr %1, !tbaa !2
  %29 = call addrspace(1) i16 @at(ptr addrspace(1) %27, i8 %28)
  ret i16 %29

b6:
  %30 = load i16, ptr %3, !tbaa !2
  %31 = load i16, ptr %2, !tbaa !2
  %32 = icmp slt i16 %30, %31
  %33 = sext i1 %32 to i8
  %34 = icmp ne i8 %33, 0
  br i1 %34, label %b7, label %b9

b7:
  %35 = load i16, ptr %5, !tbaa !2
  %36 = load i16, ptr %3, !tbaa !2
  %37 = load i16, ptr %10, !tbaa !2
  %38 = sub i16 %35, 0
  %39 = sub i16 %36, 0
  %40 = mul i16 %38, 20
  %41 = add i16 %40, %39
  %42 = getelementptr inbounds i16, ptr %9, i16 %41
  store i16 %37, ptr %42, !tbaa !2
  br label %b8

b8:
  %43 = load i16, ptr %3, !tbaa !2
  %44 = add i16 %43, 1
  store i16 %44, ptr %3, !tbaa !2
  br label %b6

b9:
  br label %b4
}

declare void @N$EBND() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
