target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @describe(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %3 = load i16, ptr addrspace(1) %2
  %4 = add i16 %1, %3
  %5 = load i16, ptr addrspace(1) %0
  %6 = add i16 %4, %5
  ret i16 %6
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
  %14 = call addrspace(1) i16 @describe(ptr addrspace(1) %13)
  ret i16 %14
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
