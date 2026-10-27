target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$f64_4000000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\00@"
@$f32_3f000000 = internal constant [4 x i8] c"\00\00\00?"

define internal cc 1005 double @half(double %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = load double, ptr @$f64_4000000000000000, !tbaa !2
  %2 = fdiv double %0, %1
  ret double %2
}

define internal cc 1005 float @value() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca float
  %1 = alloca i8
  store i8 3, ptr %1, !tbaa !2
  %2 = load float, ptr @$f32_3f000000, !tbaa !2
  store float %2, ptr %0, !tbaa !2
  %3 = load i8, ptr %1, !tbaa !2
  %4 = load float, ptr %0, !tbaa !2
  %5 = sitofp i8 %3 to float
  %6 = fmul float %5, %4
  ret float %6
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
