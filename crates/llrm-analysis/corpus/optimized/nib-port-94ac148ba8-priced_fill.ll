target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @value(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca [256 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 256, i1 false)
  br label %b2

b2:
  %2 = phi i16 [ 0, %b1 ], [ %5, %b3 ]
  %3 = icmp slt i16 %2, 64
  br i1 %3, label %b3, label %b5

b3:
  %4 = getelementptr inbounds i32, ptr %1, i16 %2
  store i32 0, ptr %4, !tbaa !2
  %5 = add i16 %2, 1
  br label %b2

b5:
  %6 = load i16, ptr addrspace(1) %0
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %8 = load ptr addrspace(1), ptr addrspace(1) %7
  br label %b6

b6:
  %9 = phi i16 [ 0, %b5 ], [ %17, %b10 ]
  %10 = phi i16 [ 0, %b5 ], [ %18, %b10 ]
  %11 = icmp slt i16 %10, 4
  br i1 %11, label %b7, label %b9

b7:
  %12 = icmp ult i16 %10, %6
  br i1 %12, label %b10, label %b11

b9:
  %13 = icmp ult i16 %9, 64
  br i1 %13, label %b12, label %b13

b10:
  %14 = shl i16 %10, 1
  %15 = getelementptr i8, ptr addrspace(1) %8, i16 %14
  %16 = load i16, ptr addrspace(1) %15
  %17 = add i16 %9, %16
  %18 = add i16 %10, 1
  br label %b6

b11:
  call addrspace(1) void @N$EBND()
  unreachable

b12:
  %19 = getelementptr inbounds i32, ptr %1, i16 %9
  store i32 5, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i32, ptr %1, i16 1
  %21 = load i32, ptr %20, !tbaa !2
  ret i32 %21

b13:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [256 x i8]
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  %3 = getelementptr inbounds i16, ptr %2, i16 0
  store i16 1, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i16, ptr %2, i16 1
  store i16 2, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i16, ptr %2, i16 2
  store i16 3, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i16, ptr %2, i16 3
  store i16 4, ptr %6, !tbaa !2
  %7 = addrspacecast ptr %2 to ptr addrspace(1)
  store i16 4, ptr %1, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 4, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %7, ptr %9, !tbaa !2
  %10 = addrspacecast ptr %1 to ptr addrspace(1)
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 256, i1 false)
  br label %11

11:
  %12 = phi i16 [ 0, %b1 ], [ %16, %14 ]
  %13 = icmp slt i16 %12, 64
  br i1 %13, label %14, label %17

14:
  %15 = getelementptr inbounds i32, ptr %0, i16 %12
  store i32 0, ptr %15
  %16 = add i16 %12, 1
  br label %11

17:
  %18 = load i16, ptr addrspace(1) %10
  %19 = getelementptr i8, ptr addrspace(1) %10, i16 4
  %20 = load ptr addrspace(1), ptr addrspace(1) %19
  br label %21

21:
  %22 = phi i16 [ 0, %17 ], [ %33, %29 ]
  %23 = phi i16 [ 0, %17 ], [ %34, %29 ]
  %24 = icmp slt i16 %23, 4
  br i1 %24, label %25, label %27

25:
  %26 = icmp ult i16 %23, %18
  br i1 %26, label %29, label %35

27:
  %28 = icmp ult i16 %22, 64
  br i1 %28, label %36, label %41

29:
  %30 = shl i16 %23, 1
  %31 = getelementptr i8, ptr addrspace(1) %20, i16 %30
  %32 = load i16, ptr addrspace(1) %31
  %33 = add i16 %22, %32
  %34 = add i16 %23, 1
  br label %21

35:
  call addrspace(1) void @N$EBND()
  unreachable

36:
  %37 = getelementptr inbounds i32, ptr %0, i16 %22
  store i32 5, ptr %37
  %38 = getelementptr inbounds i32, ptr %0, i16 1
  %39 = load i32, ptr %38
  %40 = trunc i32 %39 to i16
  ret i16 %40

41:
  call addrspace(1) void @N$EBND()
  unreachable
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$EBND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
