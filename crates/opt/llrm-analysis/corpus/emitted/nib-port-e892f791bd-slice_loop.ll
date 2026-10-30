target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca [8 x i8]
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca [10 x i8]
  store i16 5, ptr %3, !tbaa !2
  store i16 5, ptr %4, !tbaa !2
  %6 = getelementptr inbounds i16, ptr %5, i16 0
  store i16 1, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i16, ptr %5, i16 1
  store i16 2, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i16, ptr %5, i16 2
  store i16 4, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i16, ptr %5, i16 3
  store i16 8, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i16, ptr %5, i16 4
  store i16 16, ptr %10, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  %11 = addrspacecast ptr %5 to ptr addrspace(1)
  %12 = getelementptr inbounds i8, ptr addrspace(1) %11, i16 2
  store i16 3, ptr %1, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 3, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %12, ptr %14, !tbaa !2
  %15 = addrspacecast ptr %1 to ptr addrspace(1)
  %16 = load i16, ptr addrspace(1) %15
  store i16 0, ptr %0, !tbaa !2
  br label %b2

b2:
  %17 = load i16, ptr %0, !tbaa !2
  %18 = icmp ult i16 %17, %16
  br i1 %18, label %b3, label %b5

b3:
  %19 = getelementptr i8, ptr addrspace(1) %15, i16 4
  %20 = load ptr addrspace(1), ptr addrspace(1) %19, !tbaa !2
  %21 = mul i16 %17, 2
  %22 = getelementptr inbounds i8, ptr addrspace(1) %20, i16 %21
  %23 = load i16, ptr %2, !tbaa !2
  %24 = load i16, ptr addrspace(1) %22
  %25 = add i16 %23, %24
  store i16 %25, ptr %2, !tbaa !2
  br label %b4

b4:
  %26 = load i16, ptr %0, !tbaa !2
  %27 = add i16 %26, 1
  store i16 %27, ptr %0, !tbaa !2
  br label %b2

b5:
  %28 = load i16, ptr %2, !tbaa !2
  ret i16 %28
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
