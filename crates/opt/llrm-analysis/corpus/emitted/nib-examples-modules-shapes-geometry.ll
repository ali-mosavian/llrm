target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i32 @Point.shifted(ptr addrspace(1) nonnull dereferenceable(4) readonly noalias nocapture %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca [4 x i8]
  %3 = load i16, ptr addrspace(1) %0
  %4 = add i16 %3, %1
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %6 = load i16, ptr addrspace(1) %5
  store i16 %4, ptr %2, !tbaa !2
  %7 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %6, ptr %7, !tbaa !2
  %8 = addrspacecast ptr %2 to ptr addrspace(1)
  %9 = load i32, ptr addrspace(1) %8, !tbaa !2
  ret i32 %9
}

define internal cc 1005 i8 @side(ptr addrspace(1) nonnull dereferenceable(4) readonly noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i8
  %2 = load i16, ptr addrspace(1) %0
  %3 = icmp slt i16 %2, 0
  br i1 %3, label %b2, label %b3

b2:
  store i8 0, ptr %1, !tbaa !2
  br label %b4

b3:
  store i8 1, ptr %1, !tbaa !2
  br label %b4

b4:
  %4 = load i8, ptr %1, !tbaa !2
  ret i8 %4
}

define internal cc 1005 i16 @secret() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  ret i16 7
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
