target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 ptr addrspace(1) @data(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %2 = load ptr addrspace(1), ptr addrspace(1) %1
  ret ptr addrspace(1) %2
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [4 x i8]
  store i16 2, ptr %1, !tbaa !2
  store i16 2, ptr %2, !tbaa !2
  %4 = getelementptr inbounds i16, ptr %3, i16 0
  store i16 4, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i16, ptr %3, i16 1
  store i16 9, ptr %5, !tbaa !2
  %6 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 2, ptr %0, !tbaa !2
  %7 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 2, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %6, ptr %8, !tbaa !2
  %9 = addrspacecast ptr %0 to ptr addrspace(1)
  %10 = call cc 1005 addrspace(1) ptr addrspace(1) @data(ptr addrspace(1) %9)
  ret i16 0
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
