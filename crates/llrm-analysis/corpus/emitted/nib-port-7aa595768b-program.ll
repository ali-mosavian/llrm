target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$f64_4000000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\00@"
@$f64_401c000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\1C@"
@$f64_c0e0002000000000 = internal constant [8 x i8] c"\00\00\00\00 \00\E0\C0"
@$f64_40e0000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\E0@"

define internal double @half(double %0) addrspace(1) {
b1:
  %1 = load double, ptr @$f64_4000000000000000, !tbaa !2
  %2 = fdiv double %0, %1
  ret double %2
}

define internal i16 @value() addrspace(1) {
b1:
  %0 = alloca double
  store double 0.000000e+00, ptr %0
  %1 = load double, ptr @$f64_401c000000000000, !tbaa !2
  store double %1, ptr %0, !tbaa !2
  %2 = load double, ptr %0, !tbaa !2
  %3 = call addrspace(1) double @half(double %2)
  %4 = load double, ptr @$f64_c0e0002000000000, !tbaa !2
  %5 = load double, ptr @$f64_40e0000000000000, !tbaa !2
  %6 = fcmp ogt double %3, %4
  %7 = sext i1 %6 to i8
  %8 = fcmp olt double %3, %5
  %9 = sext i1 %8 to i8
  %10 = and i8 %7, %9
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b2, label %b3

b2:
  %12 = fptosi double %3 to i16
  ret i16 %12

b3:
  call addrspace(1) void @N$ECNV()
  unreachable
}

declare void @N$ECNV() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
