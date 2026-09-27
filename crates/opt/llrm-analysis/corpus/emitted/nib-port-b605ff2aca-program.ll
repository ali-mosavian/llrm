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
  %0 = alloca i8
  %1 = alloca i8
  store i8 0, ptr %0
  store i8 0, ptr %1
  store i8 -56, ptr %1, !tbaa !2
  store i8 100, ptr %0, !tbaa !2
  %2 = load i8, ptr %1, !tbaa !2
  %3 = load i8, ptr %0, !tbaa !2
  %4 = zext i8 %2 to i16
  %5 = zext i8 %3 to i16
  %6 = add i16 %4, %5
  ret i16 %6
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
