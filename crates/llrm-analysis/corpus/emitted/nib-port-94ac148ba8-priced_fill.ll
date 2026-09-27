target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @value(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca [256 x i8]
  %9 = alloca i32
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 256, i1 false)
  store i32 0, ptr %9
  store i32 0, ptr %9, !tbaa !2
  store i16 64, ptr %6, !tbaa !2
  store i16 64, ptr %7, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  store i16 64, ptr %4, !tbaa !2
  br label %b2

b2:
  %10 = load i16, ptr %5, !tbaa !2
  %11 = load i16, ptr %4, !tbaa !2
  %12 = icmp slt i16 %10, %11
  %13 = sext i1 %12 to i8
  %14 = icmp ne i8 %13, 0
  br i1 %14, label %b3, label %b5

b3:
  %15 = load i16, ptr %5, !tbaa !2
  %16 = load i32, ptr %9, !tbaa !2
  %17 = sub i16 %15, 0
  %18 = getelementptr inbounds i32, ptr %8, i16 %17
  store i32 %16, ptr %18, !tbaa !2
  br label %b4

b4:
  %19 = load i16, ptr %5, !tbaa !2
  %20 = add i16 %19, 1
  store i16 %20, ptr %5, !tbaa !2
  br label %b2

b5:
  store i16 0, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 4, ptr %1, !tbaa !2
  br label %b6

b6:
  %21 = load i16, ptr %2, !tbaa !2
  %22 = load i16, ptr %1, !tbaa !2
  %23 = icmp slt i16 %21, %22
  %24 = sext i1 %23 to i8
  %25 = icmp ne i8 %24, 0
  br i1 %25, label %b7, label %b9

b7:
  %26 = load i16, ptr %3, !tbaa !2
  %27 = load i16, ptr %2, !tbaa !2
  %28 = load i16, ptr addrspace(1) %0
  %29 = icmp ult i16 %27, %28
  %30 = sext i1 %29 to i8
  %31 = icmp ne i8 %30, 0
  br i1 %31, label %b10, label %b11

b8:
  %32 = load i16, ptr %2, !tbaa !2
  %33 = add i16 %32, 1
  store i16 %33, ptr %2, !tbaa !2
  br label %b6

b9:
  %34 = load i16, ptr %3, !tbaa !2
  %35 = icmp ult i16 %34, 64
  %36 = sext i1 %35 to i8
  %37 = icmp ne i8 %36, 0
  br i1 %37, label %b12, label %b13

b10:
  %38 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %39 = load ptr addrspace(1), ptr addrspace(1) %38
  %40 = mul i16 %27, 2
  %41 = getelementptr i8, ptr addrspace(1) %39, i16 %40
  %42 = load i16, ptr addrspace(1) %41
  %43 = add i16 %26, %42
  store i16 %43, ptr %3, !tbaa !2
  br label %b8

b11:
  call addrspace(1) void @N$EBND()
  unreachable

b12:
  %44 = sub i16 %34, 0
  %45 = getelementptr inbounds i32, ptr %8, i16 %44
  store i32 5, ptr %45, !tbaa !2
  %46 = sub i16 1, 0
  %47 = getelementptr inbounds i32, ptr %8, i16 %46
  %48 = load i32, ptr %47, !tbaa !2
  ret i32 %48

b13:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  store i16 0, ptr %1
  store i16 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  store i16 4, ptr %1, !tbaa !2
  store i16 4, ptr %2, !tbaa !2
  %4 = sub i16 0, 0
  %5 = getelementptr inbounds i16, ptr %3, i16 %4
  store i16 1, ptr %5, !tbaa !2
  %6 = sub i16 1, 0
  %7 = getelementptr inbounds i16, ptr %3, i16 %6
  store i16 2, ptr %7, !tbaa !2
  %8 = sub i16 2, 0
  %9 = getelementptr inbounds i16, ptr %3, i16 %8
  store i16 3, ptr %9, !tbaa !2
  %10 = sub i16 3, 0
  %11 = getelementptr inbounds i16, ptr %3, i16 %10
  store i16 4, ptr %11, !tbaa !2
  %12 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 4, ptr %0, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 4, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %12, ptr %14, !tbaa !2
  %15 = addrspacecast ptr %0 to ptr addrspace(1)
  %16 = call addrspace(1) i32 @value(ptr addrspace(1) %15)
  %17 = trunc i32 %16 to i16
  ret i16 %17
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$EBND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
