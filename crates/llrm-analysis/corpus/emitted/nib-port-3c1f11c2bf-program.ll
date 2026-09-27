target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$f64_4000000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\00@"

define internal double @half(double %0) addrspace(1) {
b1:
  %1 = load double, ptr @$f64_4000000000000000, !tbaa !2
  %2 = fdiv double %0, %1
  ret double %2
}

define internal i32 @value() addrspace(1) {
b1:
  %0 = alloca i16
  store i16 0, ptr %0
  store i16 1, ptr %0, !tbaa !2
  %1 = load i16, ptr %0, !tbaa !2
  %2 = sext i16 %1 to i32
  %3 = add i32 %2, 40000
  ret i32 %3
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
