target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$f64_4000000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\00@"
@$f64_401c000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\1C@"
@$f64_c0e0002000000000 = internal constant [8 x i8] c"\00\00\00\00 \00\E0\C0"
@$f64_40e0000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\E0@"

define internal double @half(double %0) addrspace(1) willreturn {
b1:
  %1 = load double, ptr @$f64_4000000000000000, !tbaa !2
  %2 = fdiv double %0, %1
  ret double %2
}

define internal i16 @value() addrspace(1) {
b1:
  %0 = load double, ptr @$f64_401c000000000000, !tbaa !2
  %1 = load double, ptr @$f64_4000000000000000
  %2 = fdiv double %0, %1
  %3 = load double, ptr @$f64_c0e0002000000000, !tbaa !2
  %4 = load double, ptr @$f64_40e0000000000000, !tbaa !2
  %5 = fcmp ogt double %2, %3
  %6 = sext i1 %5 to i8
  %7 = fcmp olt double %2, %4
  %8 = sext i1 %7 to i8
  %9 = and i8 %6, %8
  %10 = icmp ne i8 %9, 0
  br i1 %10, label %b2, label %b3

b2:
  %11 = fptosi double %2 to i16
  ret i16 %11

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
