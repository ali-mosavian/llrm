target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca [8 x i8]
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca [10 x i8]
  store i16 0, ptr %0
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 10, i1 false)
  store i16 5, ptr %3, !tbaa !2
  store i16 5, ptr %4, !tbaa !2
  %6 = sub i16 0, 0
  %7 = getelementptr inbounds i16, ptr %5, i16 %6
  store i16 1, ptr %7, !tbaa !2
  %8 = sub i16 1, 0
  %9 = getelementptr inbounds i16, ptr %5, i16 %8
  store i16 2, ptr %9, !tbaa !2
  %10 = sub i16 2, 0
  %11 = getelementptr inbounds i16, ptr %5, i16 %10
  store i16 4, ptr %11, !tbaa !2
  %12 = sub i16 3, 0
  %13 = getelementptr inbounds i16, ptr %5, i16 %12
  store i16 8, ptr %13, !tbaa !2
  %14 = sub i16 4, 0
  %15 = getelementptr inbounds i16, ptr %5, i16 %14
  store i16 16, ptr %15, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  %16 = addrspacecast ptr %5 to ptr addrspace(1)
  %17 = getelementptr i8, ptr addrspace(1) %16, i16 2
  store i16 3, ptr %1, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 3, ptr %18, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %17, ptr %19, !tbaa !2
  %20 = addrspacecast ptr %1 to ptr addrspace(1)
  %21 = load i16, ptr addrspace(1) %20
  store i16 0, ptr %0, !tbaa !2
  br label %b2

b2:
  %22 = load i16, ptr %0, !tbaa !2
  %23 = icmp ult i16 %22, %21
  %24 = sext i1 %23 to i8
  %25 = icmp ne i8 %24, 0
  br i1 %25, label %b3, label %b5

b3:
  %26 = getelementptr i8, ptr addrspace(1) %20, i16 4
  %27 = load ptr addrspace(1), ptr addrspace(1) %26, !tbaa !2
  %28 = mul i16 %22, 2
  %29 = getelementptr i8, ptr addrspace(1) %27, i16 %28
  %30 = load i16, ptr %2, !tbaa !2
  %31 = load i16, ptr addrspace(1) %29
  %32 = add i16 %30, %31
  store i16 %32, ptr %2, !tbaa !2
  br label %b4

b4:
  %33 = load i16, ptr %0, !tbaa !2
  %34 = add i16 %33, 1
  store i16 %34, ptr %0, !tbaa !2
  br label %b2

b5:
  %35 = load i16, ptr %2, !tbaa !2
  ret i16 %35
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
