target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal ptr addrspace(1) @data(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %2 = load ptr addrspace(1), ptr addrspace(1) %1
  ret ptr addrspace(1) %2
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  store i16 0, ptr %1
  store i16 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  store i16 2, ptr %1, !tbaa !2
  store i16 2, ptr %2, !tbaa !2
  %4 = sub i16 0, 0
  %5 = getelementptr inbounds i16, ptr %3, i16 %4
  store i16 4, ptr %5, !tbaa !2
  %6 = sub i16 1, 0
  %7 = getelementptr inbounds i16, ptr %3, i16 %6
  store i16 9, ptr %7, !tbaa !2
  %8 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 2, ptr %0, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 2, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %8, ptr %10, !tbaa !2
  %11 = addrspacecast ptr %0 to ptr addrspace(1)
  %12 = call addrspace(1) ptr addrspace(1) @data(ptr addrspace(1) %11)
  ret i16 0
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
