target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @total(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) willreturn {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  br label %b2

b2:
  %4 = phi i32 [ 0, %b1 ], [ %11, %b3 ]
  %5 = phi i16 [ 0, %b1 ], [ %12, %b3 ]
  %6 = icmp ult i16 %5, %1
  br i1 %6, label %b3, label %b5

b3:
  %7 = mul i16 %5, 10
  %8 = getelementptr i8, ptr addrspace(1) %3, i16 %7
  %9 = getelementptr i8, ptr addrspace(1) %8, i16 2
  %10 = load i32, ptr addrspace(1) %9
  %11 = add i32 %4, %10
  %12 = add i16 %5, 1
  br label %b2

b5:
  ret i32 %4
}

define internal i32 @update(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca [50 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 50, i1 false)
  %2 = load i16, ptr addrspace(1) %0
  %3 = icmp ugt i16 %2, 0
  br i1 %3, label %b2, label %b3

b2:
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %5 = load ptr addrspace(1), ptr addrspace(1) %4
  %6 = getelementptr i8, ptr addrspace(1) %5, i16 0
  %7 = load i32, ptr addrspace(1) %6
  %8 = icmp ugt i16 %2, 1
  br i1 %8, label %b4, label %b5

b3:
  call addrspace(1) void @N$EBND()
  unreachable

b4:
  %9 = getelementptr i8, ptr addrspace(1) %5, i16 4
  %10 = load i32, ptr addrspace(1) %9
  %11 = getelementptr inbounds [10 x i8], ptr %1, i16 0
  store i16 0, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %11, i16 2
  store i32 %7, ptr %12, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %11, i16 6
  store i32 %10, ptr %13, !tbaa !2
  %14 = icmp ugt i16 %2, 2
  br i1 %14, label %b8, label %b9

b5:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %15 = getelementptr i8, ptr addrspace(1) %5, i16 8
  %16 = load i32, ptr addrspace(1) %15
  %17 = getelementptr inbounds [10 x i8], ptr %1, i16 1
  store i16 0, ptr %17, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %17, i16 2
  store i32 %10, ptr %18, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %17, i16 6
  store i32 %16, ptr %19, !tbaa !2
  %20 = icmp ugt i16 %2, 3
  br i1 %20, label %b12, label %b13

b9:
  call addrspace(1) void @N$EBND()
  unreachable

b12:
  %21 = getelementptr i8, ptr addrspace(1) %5, i16 12
  %22 = load i32, ptr addrspace(1) %21
  %23 = getelementptr inbounds [10 x i8], ptr %1, i16 2
  store i16 0, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %23, i16 2
  store i32 %16, ptr %24, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %23, i16 6
  store i32 %22, ptr %25, !tbaa !2
  %26 = icmp ugt i16 %2, 4
  br i1 %26, label %b16, label %b17

b13:
  call addrspace(1) void @N$EBND()
  unreachable

b16:
  %27 = getelementptr i8, ptr addrspace(1) %5, i16 16
  %28 = load i32, ptr addrspace(1) %27
  %29 = getelementptr inbounds [10 x i8], ptr %1, i16 3
  store i16 0, ptr %29, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %29, i16 2
  store i32 %22, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %29, i16 6
  store i32 %28, ptr %31, !tbaa !2
  %32 = icmp ugt i16 %2, 5
  br i1 %32, label %b20, label %b21

b17:
  call addrspace(1) void @N$EBND()
  unreachable

b20:
  %33 = getelementptr i8, ptr addrspace(1) %5, i16 20
  %34 = load i32, ptr addrspace(1) %33
  %35 = getelementptr inbounds [10 x i8], ptr %1, i16 4
  store i16 0, ptr %35, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %35, i16 2
  store i32 %28, ptr %36, !tbaa !2
  %37 = getelementptr inbounds i8, ptr %35, i16 6
  store i32 %34, ptr %37, !tbaa !2
  br label %b22

b21:
  call addrspace(1) void @N$EBND()
  unreachable

b22:
  %38 = phi i16 [ 0, %b20 ], [ %46, %b23 ]
  %39 = icmp ult i16 %38, 5
  br i1 %39, label %b23, label %b25

b23:
  %40 = getelementptr inbounds [10 x i8], ptr %1, i16 %38
  %41 = getelementptr inbounds i8, ptr %40, i16 2
  %42 = load i32, ptr %41, !tbaa !2
  %43 = getelementptr inbounds i8, ptr %40, i16 6
  %44 = load i32, ptr %43, !tbaa !2
  %45 = add i32 %42, %44
  store i32 %45, ptr %41, !tbaa !2
  %46 = add i16 %38, 1
  br label %b22

b25:
  %47 = addrspacecast ptr %1 to ptr addrspace(1)
  br label %48

48:
  %49 = phi i32 [ 0, %b25 ], [ %57, %52 ]
  %50 = phi i16 [ 0, %b25 ], [ %58, %52 ]
  %51 = icmp ult i16 %50, 5
  br i1 %51, label %52, label %59

52:
  %53 = mul i16 %50, 10
  %54 = getelementptr i8, ptr addrspace(1) %47, i16 %53
  %55 = getelementptr i8, ptr addrspace(1) %54, i16 2
  %56 = load i32, ptr addrspace(1) %55
  %57 = add i32 %49, %56
  %58 = add i16 %50, 1
  br label %48

59:
  ret i32 %49
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca [24 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 24, i1 false)
  %2 = getelementptr inbounds i32, ptr %1, i16 0
  store i32 1, ptr %2, !tbaa !2
  %3 = getelementptr inbounds i32, ptr %1, i16 1
  store i32 2, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i32, ptr %1, i16 2
  store i32 3, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i32, ptr %1, i16 3
  store i32 4, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i32, ptr %1, i16 4
  store i32 5, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i32, ptr %1, i16 5
  store i32 6, ptr %7, !tbaa !2
  %8 = addrspacecast ptr %1 to ptr addrspace(1)
  store i16 6, ptr %0, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 6, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %8, ptr %10, !tbaa !2
  %11 = addrspacecast ptr %0 to ptr addrspace(1)
  %12 = call addrspace(1) i32 @update(ptr addrspace(1) %11)
  ret i16 0
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$EBND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
