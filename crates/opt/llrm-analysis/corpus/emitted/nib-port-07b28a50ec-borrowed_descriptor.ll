target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i16 @describe(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %3 = load i16, ptr addrspace(1) %2
  %4 = add i16 %1, %3
  %5 = load i16, ptr addrspace(1) %0
  %6 = add i16 %4, %5
  ret i16 %6
}

define internal cc 1005 i16 @calculate() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [6 x i8]
  store i16 3, ptr %1, !tbaa !2
  store i16 3, ptr %2, !tbaa !2
  %4 = getelementptr inbounds i16, ptr %3, i16 0
  store i16 10, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i16, ptr %3, i16 1
  store i16 20, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i16, ptr %3, i16 2
  store i16 30, ptr %6, !tbaa !2
  %7 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %7, ptr %9, !tbaa !2
  %10 = addrspacecast ptr %0 to ptr addrspace(1)
  %11 = call cc 1005 addrspace(1) i16 @describe(ptr addrspace(1) %10)
  ret i16 %11
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
