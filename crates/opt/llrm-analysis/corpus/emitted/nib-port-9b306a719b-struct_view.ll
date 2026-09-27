target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @total(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i32
  store i16 0, ptr %1
  store i32 0, ptr %2
  store i32 0, ptr %2, !tbaa !2
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
  %10 = mul i16 %4, 10
  %11 = getelementptr i8, ptr addrspace(1) %9, i16 %10
  %12 = load i32, ptr %2, !tbaa !2
  %13 = getelementptr i8, ptr addrspace(1) %11, i16 2
  %14 = load i32, ptr addrspace(1) %13
  %15 = add i32 %12, %14
  store i32 %15, ptr %2, !tbaa !2
  br label %b4

b4:
  %16 = load i16, ptr %1, !tbaa !2
  %17 = add i16 %16, 1
  store i16 %17, ptr %1, !tbaa !2
  br label %b2

b5:
  %18 = load i32, ptr %2, !tbaa !2
  ret i32 %18
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [20 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  store i16 0, ptr %1
  store i16 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 20, i1 false)
  store i16 2, ptr %1, !tbaa !2
  store i16 2, ptr %2, !tbaa !2
  %4 = sub i16 0, 0
  %5 = getelementptr inbounds [10 x i8], ptr %3, i16 %4
  store i16 0, ptr %5, !tbaa !2
  %6 = sub i16 0, 0
  %7 = getelementptr inbounds [10 x i8], ptr %3, i16 %6
  %8 = getelementptr inbounds i8, ptr %7, i16 2
  store i32 1, ptr %8, !tbaa !2
  %9 = sub i16 0, 0
  %10 = getelementptr inbounds [10 x i8], ptr %3, i16 %9
  %11 = getelementptr inbounds i8, ptr %10, i16 6
  store i32 2, ptr %11, !tbaa !2
  %12 = sub i16 1, 0
  %13 = getelementptr inbounds [10 x i8], ptr %3, i16 %12
  store i16 0, ptr %13, !tbaa !2
  %14 = sub i16 1, 0
  %15 = getelementptr inbounds [10 x i8], ptr %3, i16 %14
  %16 = getelementptr inbounds i8, ptr %15, i16 2
  store i32 2, ptr %16, !tbaa !2
  %17 = sub i16 1, 0
  %18 = getelementptr inbounds [10 x i8], ptr %3, i16 %17
  %19 = getelementptr inbounds i8, ptr %18, i16 6
  store i32 3, ptr %19, !tbaa !2
  %20 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 2, ptr %0, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 2, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %20, ptr %22, !tbaa !2
  %23 = addrspacecast ptr %0 to ptr addrspace(1)
  %24 = call addrspace(1) i32 @total(ptr addrspace(1) %23)
  %25 = trunc i32 %24 to i16
  ret i16 %25
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
