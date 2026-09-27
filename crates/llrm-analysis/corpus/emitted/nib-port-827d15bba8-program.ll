target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$f64_4000000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\00@"

define internal double @half(double %0) addrspace(1) {
b1:
  %1 = load double, ptr @$f64_4000000000000000, !tbaa !2
  %2 = fdiv double %0, %1
  ret double %2
}

define internal i16 @value() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i8
  %4 = alloca i16
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i8 0, ptr %3
  store i16 0, ptr %4
  store i16 1, ptr %4, !tbaa !2
  store i8 4, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  %5 = load i16, ptr %4, !tbaa !2
  %6 = load i8, ptr %3, !tbaa !2
  %7 = zext i8 %6 to i16
  store i16 %5, ptr %1, !tbaa !2
  store i16 %7, ptr %0, !tbaa !2
  br label %b2

b2:
  %8 = load i16, ptr %1, !tbaa !2
  %9 = load i16, ptr %0, !tbaa !2
  %10 = icmp slt i16 %8, %9
  %11 = sext i1 %10 to i8
  %12 = icmp ne i8 %11, 0
  br i1 %12, label %b3, label %b5

b3:
  %13 = load i16, ptr %2, !tbaa !2
  %14 = load i16, ptr %1, !tbaa !2
  %15 = add i16 %13, %14
  store i16 %15, ptr %2, !tbaa !2
  br label %b4

b4:
  %16 = load i16, ptr %1, !tbaa !2
  %17 = add i16 %16, 1
  store i16 %17, ptr %1, !tbaa !2
  br label %b2

b5:
  %18 = load i16, ptr %2, !tbaa !2
  ret i16 %18
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
