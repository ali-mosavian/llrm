target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 void @nudge(ptr addrspace(1) nonnull dereferenceable(4) noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %3 = load i16, ptr addrspace(1) %2
  %4 = add i16 %1, %3
  store i16 %4, ptr addrspace(1) %0
  ret void
}

define internal cc 1005 void @update(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %3 = load i16, ptr %1, !tbaa !2
  %4 = icmp ult i16 %3, %2
  br i1 %4, label %b3, label %b5

b3:
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %6 = load ptr addrspace(1), ptr addrspace(1) %5
  %7 = mul i16 %3, 4
  %8 = getelementptr inbounds i8, ptr addrspace(1) %6, i16 %7
  call cc 1005 addrspace(1) void @nudge(ptr addrspace(1) %8)
  br label %b4

b4:
  %9 = load i16, ptr %1, !tbaa !2
  %10 = add i16 %9, 1
  store i16 %10, ptr %1, !tbaa !2
  br label %b2

b5:
  ret void
}

define internal cc 1005 i16 @calculate() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [8 x i8]
  store i16 2, ptr %1, !tbaa !2
  store i16 2, ptr %2, !tbaa !2
  %4 = getelementptr inbounds [4 x i8], ptr %3, i16 0
  store i16 1, ptr %4, !tbaa !2
  %5 = getelementptr inbounds [4 x i8], ptr %3, i16 0
  %6 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 2, ptr %6, !tbaa !2
  %7 = getelementptr inbounds [4 x i8], ptr %3, i16 1
  store i16 10, ptr %7, !tbaa !2
  %8 = getelementptr inbounds [4 x i8], ptr %3, i16 1
  %9 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 20, ptr %9, !tbaa !2
  %10 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 2, ptr %0, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 2, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %10, ptr %12, !tbaa !2
  %13 = addrspacecast ptr %0 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @update(ptr addrspace(1) %13)
  %14 = getelementptr inbounds [4 x i8], ptr %3, i16 0
  %15 = load i16, ptr %14, !tbaa !2
  %16 = getelementptr inbounds [4 x i8], ptr %3, i16 1
  %17 = load i16, ptr %16, !tbaa !2
  %18 = add i16 %15, %17
  ret i16 %18
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
