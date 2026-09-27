target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal void @nudge(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %3 = load i16, ptr addrspace(1) %2
  %4 = add i16 %1, %3
  store i16 %4, ptr addrspace(1) %0
  ret void
}

define internal void @update(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca i16
  store i16 0, ptr %1
  %2 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %3 = load i16, ptr %1, !tbaa !2
  %4 = icmp ult i16 %3, %2
  %5 = sext i1 %4 to i8
  %6 = icmp ne i8 %5, 0
  br i1 %6, label %b3, label %b5

b3:
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %8 = load ptr addrspace(1), ptr addrspace(1) %7
  %9 = mul i16 %3, 4
  %10 = getelementptr i8, ptr addrspace(1) %8, i16 %9
  call addrspace(1) void @nudge(ptr addrspace(1) %10)
  br label %b4

b4:
  %11 = load i16, ptr %1, !tbaa !2
  %12 = add i16 %11, 1
  store i16 %12, ptr %1, !tbaa !2
  br label %b2

b5:
  ret void
}

define internal i16 @calculate() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  store i16 0, ptr %1
  store i16 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  store i16 2, ptr %1, !tbaa !2
  store i16 2, ptr %2, !tbaa !2
  %4 = sub i16 0, 0
  %5 = getelementptr inbounds [4 x i8], ptr %3, i16 %4
  store i16 1, ptr %5, !tbaa !2
  %6 = sub i16 0, 0
  %7 = getelementptr inbounds [4 x i8], ptr %3, i16 %6
  %8 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 2, ptr %8, !tbaa !2
  %9 = sub i16 1, 0
  %10 = getelementptr inbounds [4 x i8], ptr %3, i16 %9
  store i16 10, ptr %10, !tbaa !2
  %11 = sub i16 1, 0
  %12 = getelementptr inbounds [4 x i8], ptr %3, i16 %11
  %13 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 20, ptr %13, !tbaa !2
  %14 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 2, ptr %0, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 2, ptr %15, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %14, ptr %16, !tbaa !2
  %17 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @update(ptr addrspace(1) %17)
  %18 = sub i16 0, 0
  %19 = getelementptr inbounds [4 x i8], ptr %3, i16 %18
  %20 = load i16, ptr %19, !tbaa !2
  %21 = sub i16 1, 0
  %22 = getelementptr inbounds [4 x i8], ptr %3, i16 %21
  %23 = load i16, ptr %22, !tbaa !2
  %24 = add i16 %20, %23
  ret i16 %24
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
