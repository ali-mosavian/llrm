target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i32 @total(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i32
  store i32 0, ptr %2, !tbaa !2
  %3 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %1, !tbaa !2
  %5 = icmp ult i16 %4, %3
  br i1 %5, label %b3, label %b5

b3:
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %7 = load ptr addrspace(1), ptr addrspace(1) %6
  %8 = mul i16 %4, 10
  %9 = getelementptr inbounds i8, ptr addrspace(1) %7, i16 %8
  %10 = load i32, ptr %2, !tbaa !2
  %11 = getelementptr i8, ptr addrspace(1) %9, i16 2
  %12 = load i32, ptr addrspace(1) %11
  %13 = add i32 %10, %12
  store i32 %13, ptr %2, !tbaa !2
  br label %b4

b4:
  %14 = load i16, ptr %1, !tbaa !2
  %15 = add i16 %14, 1
  store i16 %15, ptr %1, !tbaa !2
  br label %b2

b5:
  %16 = load i32, ptr %2, !tbaa !2
  ret i32 %16
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [20 x i8]
  store i16 2, ptr %1, !tbaa !2
  store i16 2, ptr %2, !tbaa !2
  %4 = getelementptr inbounds [10 x i8], ptr %3, i16 0
  store i16 0, ptr %4, !tbaa !2
  %5 = getelementptr inbounds [10 x i8], ptr %3, i16 0
  %6 = getelementptr inbounds i8, ptr %5, i16 2
  store i32 1, ptr %6, !tbaa !2
  %7 = getelementptr inbounds [10 x i8], ptr %3, i16 0
  %8 = getelementptr inbounds i8, ptr %7, i16 6
  store i32 2, ptr %8, !tbaa !2
  %9 = getelementptr inbounds [10 x i8], ptr %3, i16 1
  store i16 0, ptr %9, !tbaa !2
  %10 = getelementptr inbounds [10 x i8], ptr %3, i16 1
  %11 = getelementptr inbounds i8, ptr %10, i16 2
  store i32 2, ptr %11, !tbaa !2
  %12 = getelementptr inbounds [10 x i8], ptr %3, i16 1
  %13 = getelementptr inbounds i8, ptr %12, i16 6
  store i32 3, ptr %13, !tbaa !2
  %14 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 2, ptr %0, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 2, ptr %15, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %14, ptr %16, !tbaa !2
  %17 = addrspacecast ptr %0 to ptr addrspace(1)
  %18 = call cc 1005 addrspace(1) i32 @total(ptr addrspace(1) %17)
  %19 = trunc i32 %18 to i16
  ret i16 %19
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
