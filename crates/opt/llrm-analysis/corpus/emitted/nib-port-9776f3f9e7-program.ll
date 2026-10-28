target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$f64_4000000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\00@"

define internal cc 1005 double @half(double %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = load double, ptr @$f64_4000000000000000, !tbaa !2
  %2 = fdiv double %0, %1
  ret double %2
}

define internal cc 1005 i32 @value() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i32
  store i32 15, ptr %0, !tbaa !2
  %1 = load i32, ptr %0, !tbaa !2
  %2 = icmp ult i32 %1, 16
  br i1 %2, label %b2, label %b3

b2:
  %3 = trunc i32 %1 to i16
  %4 = shl i16 1, %3
  %5 = sext i16 %4 to i32
  ret i32 %5

b3:
  call cc 1005 addrspace(1) void @N$ESHF()
  unreachable
}

declare cc 1005 void @N$ESHF() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
