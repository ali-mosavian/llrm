target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i16 @value() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca [6 x i8]
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca [4 x i8]
  store i16 2, ptr %6, !tbaa !2
  store i16 2, ptr %7, !tbaa !2
  %9 = getelementptr inbounds i16, ptr %8, i16 0
  store i16 5, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i16, ptr %8, i16 1
  store i16 6, ptr %10, !tbaa !2
  br i1 true, label %b2, label %b3

b2:
  %11 = getelementptr inbounds i16, ptr %8, i16 0
  %12 = load i16, ptr %11, !tbaa !2
  store i16 %12, ptr %5, !tbaa !2
  store i16 3, ptr %2, !tbaa !2
  store i16 3, ptr %3, !tbaa !2
  store i16 0, ptr %1, !tbaa !2
  store i16 3, ptr %0, !tbaa !2
  br label %b5

b3:
  br label %b4

b4:
  ret i16 0

b5:
  %13 = load i16, ptr %1, !tbaa !2
  %14 = load i16, ptr %0, !tbaa !2
  %15 = icmp slt i16 %13, %14
  br i1 %15, label %b6, label %b8

b6:
  %16 = load i16, ptr %1, !tbaa !2
  %17 = load i16, ptr %5, !tbaa !2
  %18 = sub i16 %16, 0
  %19 = getelementptr inbounds i16, ptr %4, i16 %18
  store i16 %17, ptr %19, !tbaa !2
  br label %b7

b7:
  %20 = load i16, ptr %1, !tbaa !2
  %21 = add nsw i16 %20, 1
  store i16 %21, ptr %1, !tbaa !2
  br label %b5

b8:
  %22 = getelementptr inbounds i16, ptr %4, i16 0
  %23 = load i16, ptr %22, !tbaa !2
  %24 = getelementptr inbounds i16, ptr %4, i16 2
  %25 = load i16, ptr %24, !tbaa !2
  %26 = add i16 %23, %25
  ret i16 %26
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
