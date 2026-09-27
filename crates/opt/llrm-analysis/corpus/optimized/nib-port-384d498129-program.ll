target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$f64_4000000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\00@"
@$f32_3f000000 = internal constant [4 x i8] c"\00\00\00?"

define internal double @half(double %0) addrspace(1) willreturn {
b1:
  %1 = load double, ptr @$f64_4000000000000000, !tbaa !2
  %2 = fdiv double %0, %1
  ret double %2
}

define internal float @value() addrspace(1) willreturn {
b1:
  %0 = load float, ptr @$f32_3f000000, !tbaa !2
  %1 = fmul float 3.000000e+00, %0
  ret float %1
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
