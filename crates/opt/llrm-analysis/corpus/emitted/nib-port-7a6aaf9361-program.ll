target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$f64_4000000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\00@"

define internal cc 1005 double @half(double %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = load double, ptr @$f64_4000000000000000, !tbaa !2
  %2 = fdiv double %0, %1
  ret double %2
}

define internal cc 1005 i16 @value() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i8
  store i8 3, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  %4 = load i8, ptr %3, !tbaa !2
  %5 = load i8, ptr %3, !tbaa !2
  %6 = zext i8 %4 to i16
  %7 = zext i8 %5 to i16
  %8 = add i16 %6, %7
  store i16 0, ptr %1, !tbaa !2
  store i16 %8, ptr %0, !tbaa !2
  br label %b2

b2:
  %9 = load i16, ptr %1, !tbaa !2
  %10 = load i16, ptr %0, !tbaa !2
  %11 = icmp slt i16 %9, %10
  br i1 %11, label %b3, label %b5

b3:
  %12 = load i16, ptr %2, !tbaa !2
  %13 = load i16, ptr %1, !tbaa !2
  %14 = add i16 %12, %13
  store i16 %14, ptr %2, !tbaa !2
  br label %b4

b4:
  %15 = load i16, ptr %1, !tbaa !2
  %16 = add nsw i16 %15, 1
  store i16 %16, ptr %1, !tbaa !2
  br label %b2

b5:
  %17 = load i16, ptr %2, !tbaa !2
  ret i16 %17
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
