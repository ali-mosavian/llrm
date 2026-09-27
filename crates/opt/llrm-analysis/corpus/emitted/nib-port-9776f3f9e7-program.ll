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
  %0 = alloca i32
  store i32 0, ptr %0
  store i32 15, ptr %0, !tbaa !2
  %1 = load i32, ptr %0, !tbaa !2
  %2 = icmp ult i32 %1, 16
  %3 = sext i1 %2 to i8
  %4 = icmp ne i8 %3, 0
  br i1 %4, label %b2, label %b3

b2:
  %5 = trunc i32 %1 to i16
  %6 = shl i16 1, %5
  %7 = sext i16 %6 to i32
  ret i32 %7

b3:
  call addrspace(1) void @N$ESHF()
  unreachable
}

declare void @N$ESHF() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
