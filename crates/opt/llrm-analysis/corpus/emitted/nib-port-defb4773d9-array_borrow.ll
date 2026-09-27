target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal void @bump(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = icmp ult i16 1, %1
  %3 = sext i1 %2 to i8
  %4 = icmp ne i8 %3, 0
  br i1 %4, label %b2, label %b3

b2:
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %6 = load ptr addrspace(1), ptr addrspace(1) %5
  %7 = getelementptr i8, ptr addrspace(1) %6, i16 2
  %8 = load i16, ptr addrspace(1) %7
  %9 = load i16, ptr addrspace(1) %0
  %10 = add i16 %8, %9
  store i16 %10, ptr addrspace(1) %7
  ret void

b3:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @calculate() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [6 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  store i16 0, ptr %1
  store i16 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 6, i1 false)
  store i16 3, ptr %1, !tbaa !2
  store i16 3, ptr %2, !tbaa !2
  %4 = sub i16 0, 0
  %5 = getelementptr inbounds i16, ptr %3, i16 %4
  store i16 10, ptr %5, !tbaa !2
  %6 = sub i16 1, 0
  %7 = getelementptr inbounds i16, ptr %3, i16 %6
  store i16 20, ptr %7, !tbaa !2
  %8 = sub i16 2, 0
  %9 = getelementptr inbounds i16, ptr %3, i16 %8
  store i16 30, ptr %9, !tbaa !2
  %10 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %10, ptr %12, !tbaa !2
  %13 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @bump(ptr addrspace(1) %13)
  %14 = sub i16 0, 0
  %15 = getelementptr inbounds i16, ptr %3, i16 %14
  %16 = load i16, ptr %15, !tbaa !2
  %17 = sub i16 1, 0
  %18 = getelementptr inbounds i16, ptr %3, i16 %17
  %19 = load i16, ptr %18, !tbaa !2
  %20 = add i16 %16, %19
  %21 = sub i16 2, 0
  %22 = getelementptr inbounds i16, ptr %3, i16 %21
  %23 = load i16, ptr %22, !tbaa !2
  %24 = add i16 %20, %23
  ret i16 %24
}

declare void @N$EBND() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
