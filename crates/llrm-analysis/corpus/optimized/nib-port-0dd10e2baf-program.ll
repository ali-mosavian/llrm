target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$f64_401f99999999999a = internal constant [8 x i8] c"\9A\99\99\99\99\99\1F@"
@$f64_c0e0002000000000 = internal constant [8 x i8] c"\00\00\00\00 \00\E0\C0"
@$f64_40e0000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\E0@"

define internal i16 @value() addrspace(1) {
b1:
  %0 = load double, ptr @$f64_401f99999999999a, !tbaa !2
  %1 = fneg double %0
  %2 = load double, ptr @$f64_c0e0002000000000, !tbaa !2
  %3 = load double, ptr @$f64_40e0000000000000, !tbaa !2
  %4 = fcmp ogt double %1, %2
  %5 = sext i1 %4 to i8
  %6 = fcmp olt double %1, %3
  %7 = sext i1 %6 to i8
  %8 = and i8 %5, %7
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b2, label %b3

b2:
  %10 = fptosi double %1 to i16
  ret i16 %10

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
