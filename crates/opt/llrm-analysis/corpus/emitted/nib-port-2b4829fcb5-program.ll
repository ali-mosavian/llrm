target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i32 @value() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i32
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca [20 x i8]
  %7 = alloca i32
  store i32 7, ptr %7, !tbaa !2
  store i16 5, ptr %4, !tbaa !2
  store i16 5, ptr %5, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  store i16 5, ptr %2, !tbaa !2
  br label %b2

b2:
  %8 = load i16, ptr %3, !tbaa !2
  %9 = load i16, ptr %2, !tbaa !2
  %10 = icmp slt i16 %8, %9
  br i1 %10, label %b3, label %b5

b3:
  %11 = load i16, ptr %3, !tbaa !2
  %12 = load i32, ptr %7, !tbaa !2
  %13 = sub i16 %11, 0
  %14 = getelementptr inbounds i32, ptr %6, i16 %13
  store i32 %12, ptr %14, !tbaa !2
  br label %b4

b4:
  %15 = load i16, ptr %3, !tbaa !2
  %16 = add nsw i16 %15, 1
  store i16 %16, ptr %3, !tbaa !2
  br label %b2

b5:
  %17 = getelementptr inbounds i32, ptr %6, i16 2
  store i32 1, ptr %17, !tbaa !2
  store i32 0, ptr %1, !tbaa !2
  store i16 0, ptr %0, !tbaa !2
  br label %b6

b6:
  %18 = load i16, ptr %0, !tbaa !2
  %19 = icmp ult i16 %18, 5
  br i1 %19, label %b7, label %b9

b7:
  %20 = load i32, ptr %1, !tbaa !2
  %21 = sub i16 %18, 0
  %22 = getelementptr inbounds i32, ptr %6, i16 %21
  %23 = load i32, ptr %22, !tbaa !2
  %24 = add i32 %20, %23
  store i32 %24, ptr %1, !tbaa !2
  br label %b8

b8:
  %25 = load i16, ptr %0, !tbaa !2
  %26 = add i16 %25, 1
  store i16 %26, ptr %0, !tbaa !2
  br label %b6

b9:
  %27 = load i32, ptr %1, !tbaa !2
  ret i32 %27
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
