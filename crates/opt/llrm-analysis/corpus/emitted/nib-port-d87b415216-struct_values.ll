target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i16 @calculate() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [4 x i8]
  %1 = alloca [4 x i8]
  store i16 1, ptr %1, !tbaa !2
  %2 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 2, ptr %2, !tbaa !2
  %3 = load i16, ptr %1, !tbaa !2
  %4 = getelementptr inbounds i8, ptr %1, i16 2
  %5 = load i16, ptr %4, !tbaa !2
  store i16 %3, ptr %0, !tbaa !2
  %6 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 %5, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i8, ptr %1, i16 2
  %8 = load i16, ptr %7, !tbaa !2
  %9 = load i16, ptr %1, !tbaa !2
  store i16 %8, ptr %1, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %9, ptr %10, !tbaa !2
  %11 = load i16, ptr %1, !tbaa !2
  %12 = load i16, ptr %0, !tbaa !2
  %13 = add i16 %11, %12
  store i16 %13, ptr %1, !tbaa !2
  %14 = load i16, ptr %1, !tbaa !2
  %15 = mul i16 %14, 10
  %16 = getelementptr inbounds i8, ptr %1, i16 2
  %17 = load i16, ptr %16, !tbaa !2
  %18 = add i16 %15, %17
  ret i16 %18
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
