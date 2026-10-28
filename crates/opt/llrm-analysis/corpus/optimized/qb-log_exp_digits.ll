target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"X#" = internal global [8 x i8] zeroinitializer

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  br label %b5

b5:
  %0 = phi float [ 1.000000e+00, %b1 ], [ %19, %b14 ]
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDR8(ptr addrspace(1) addrspacecast (ptr @"X#" to ptr addrspace(1)))
  %1 = load double, ptr @"X#", !tbaa !2
  %2 = fcmp ole double %1, 0.000000e+00
  br i1 %2, label %b7, label %b8

b6:
  ret void

b7:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b8:
  %3 = tail call double @llvm.log.f64(double %1)
  %4 = fpround double %3 to double
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSR8(double %4)
  %5 = fdiv double %1, 1.000000e+01
  %6 = fcmp oge double %5, 0x40D62E2CD0B73AF5
  br i1 %6, label %b9, label %b10

b9:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b10:
  %7 = tail call double @llvm.exp.f64(double %5)
  %8 = fpround double %7 to double
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSR8(double %8)
  %9 = fmul double %1, 3.700000e+00
  %10 = fcmp ole double %9, 0.000000e+00
  br i1 %10, label %b11, label %b12

b11:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b12:
  %11 = tail call double @llvm.log.f64(double %9)
  %12 = fpround double %11 to double
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSR8(double %12)
  %13 = fneg double %1
  %14 = fdiv double %13, 1.000000e+02
  %15 = fcmp oge double %14, 0x40D62E2CD0B73AF5
  br i1 %15, label %b13, label %b14

b13:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b14:
  %16 = tail call double @llvm.exp.f64(double %14)
  %17 = fpround double %16 to double
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double %17)
  %18 = fadd float %0, 1.000000e+00
  %19 = fpround float %18 to float
  %20 = fcmp ole float %19, 8.000000e+00
  br i1 %20, label %b5, label %b6
}

declare cc 1005 void @llrm.qb.B$RDR8(ptr addrspace(1) nocapture noretain) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) nocallback noreturn

declare double @llvm.log.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @llrm.qb.B$PSR8(double) addrspace(1) nocallback

declare double @llvm.exp.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @llrm.qb.B$PER8(double) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
