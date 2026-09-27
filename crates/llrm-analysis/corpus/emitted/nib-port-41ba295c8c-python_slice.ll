target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @sum(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %2, !tbaa !2
  %3 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %1, !tbaa !2
  %5 = icmp ult i16 %4, %3
  %6 = sext i1 %5 to i8
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b3, label %b5

b3:
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %9 = load ptr addrspace(1), ptr addrspace(1) %8
  %10 = mul i16 %4, 2
  %11 = getelementptr i8, ptr addrspace(1) %9, i16 %10
  %12 = load i16, ptr %2, !tbaa !2
  %13 = load i16, ptr addrspace(1) %11
  %14 = add i16 %12, %13
  store i16 %14, ptr %2, !tbaa !2
  br label %b4

b4:
  %15 = load i16, ptr %1, !tbaa !2
  %16 = add i16 %15, 1
  store i16 %16, ptr %1, !tbaa !2
  br label %b2

b5:
  %17 = load i16, ptr %2, !tbaa !2
  ret i16 %17
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
  %13 = getelementptr i8, ptr addrspace(1) %12, i16 2
  store i16 2, ptr %0, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 2, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %13, ptr %15, !tbaa !2
  %16 = addrspacecast ptr %0 to ptr addrspace(1)
  %17 = call addrspace(1) i16 @sum(ptr addrspace(1) %16)
  ret i16 %17
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
