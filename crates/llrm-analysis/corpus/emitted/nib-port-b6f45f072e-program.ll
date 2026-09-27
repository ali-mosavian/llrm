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
  %0 = alloca i8
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca [800 x i8]
  %9 = alloca i16
  store i8 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 800, i1 false)
  store i16 0, ptr %9
  store i16 0, ptr %9, !tbaa !2
  store i16 20, ptr %5, !tbaa !2
  store i16 20, ptr %6, !tbaa !2
  store i16 400, ptr %7, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  store i16 20, ptr %3, !tbaa !2
  br label %b2

b2:
  %10 = load i16, ptr %4, !tbaa !2
  %11 = load i16, ptr %3, !tbaa !2
  %12 = icmp slt i16 %10, %11
  %13 = sext i1 %12 to i8
  %14 = icmp ne i8 %13, 0
  br i1 %14, label %b3, label %b5

b3:
  store i16 0, ptr %2, !tbaa !2
  store i16 20, ptr %1, !tbaa !2
  br label %b6

b4:
  %15 = load i16, ptr %4, !tbaa !2
  %16 = add i16 %15, 1
  store i16 %16, ptr %4, !tbaa !2
  br label %b2

b5:
  %17 = sub i16 19, 0
  %18 = sub i16 1, 0
  %19 = mul i16 %17, 20
  %20 = add i16 %19, %18
  %21 = getelementptr inbounds i16, ptr %8, i16 %20
  store i16 7, ptr %21, !tbaa !2
  store i8 19, ptr %0, !tbaa !2
  %22 = load i8, ptr %0, !tbaa !2
  %23 = zext i8 %22 to i16
  %24 = icmp ult i16 %23, 20
  %25 = sext i1 %24 to i8
  %26 = icmp ne i8 %25, 0
  br i1 %26, label %b10, label %b11

b6:
  %27 = load i16, ptr %2, !tbaa !2
  %28 = load i16, ptr %1, !tbaa !2
  %29 = icmp slt i16 %27, %28
  %30 = sext i1 %29 to i8
  %31 = icmp ne i8 %30, 0
  br i1 %31, label %b7, label %b9

b7:
  %32 = load i16, ptr %4, !tbaa !2
  %33 = load i16, ptr %2, !tbaa !2
  %34 = load i16, ptr %9, !tbaa !2
  %35 = sub i16 %32, 0
  %36 = sub i16 %33, 0
  %37 = mul i16 %35, 20
  %38 = add i16 %37, %36
  %39 = getelementptr inbounds i16, ptr %8, i16 %38
  store i16 %34, ptr %39, !tbaa !2
  br label %b8

b8:
  %40 = load i16, ptr %2, !tbaa !2
  %41 = add i16 %40, 1
  store i16 %41, ptr %2, !tbaa !2
  br label %b6

b9:
  br label %b4

b10:
  %42 = zext i8 %22 to i16
  %43 = sub i16 %42, 0
  %44 = sub i16 1, 0
  %45 = mul i16 %43, 20
  %46 = add i16 %45, %44
  %47 = getelementptr inbounds i16, ptr %8, i16 %46
  %48 = load i16, ptr %47, !tbaa !2
  ret i16 %48

b11:
  call addrspace(1) void @N$EBND()
  unreachable
}

declare void @N$EBND() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
